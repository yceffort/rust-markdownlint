#!/usr/bin/env python3
"""Build isolated musl variants, verify outputs, and time balanced permutations."""

import argparse
import datetime
import hashlib
import io
import itertools
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tarfile
import time

BASE = "a039aee926d759141829ae711ae5cd39343ca503"
TARGET = "x86_64-unknown-linux-musl"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--corpora", type=Path, required=True)
    parser.add_argument("--archive", type=Path, help="Optional git archive of BASE")
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    source = args.work / "source"
    source.mkdir(exist_ok=True)
    bins = args.work / "bin"
    bins.mkdir(exist_ok=True)
    patches = json.loads((Path(__file__).resolve().parent / "patches.json").read_text())
    variants = {"baseline": [], "output": ["output"], "parser": ["parser"], "both": ["output", "parser"]}
    report = {
        "base_commit": BASE,
        "date_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "target": TARGET,
        "environment": {k: subprocess.check_output(cmd, text=True).strip() for k, cmd in {
            "rustc": ["rustc", "--version"], "cargo": ["cargo", "--version"],
            "cpu": ["lscpu"], "kernel": ["uname", "-a"],
        }.items()},
        "method": {"runs": 24, "warmup": 3, "order": "all 24 permutations of four variants",
                   "timing": "blocking subprocess wait, stdout/stderr to /dev/null, all builds completed first"},
        "patches": {name: sha(patches[name].encode()) for name in ["output", "parser"]},
        "binaries": {}, "corpora": [],
    }
    report["environment"]["rayon_num_threads"] = os.environ.get("RAYON_NUM_THREADS")

    def state(stage):
        (args.work / "status").write_text(stage + "\n")
        print(stage, flush=True)

    def save():
        (args.work / "results.json").write_text(json.dumps(report, indent=2) + "\n")

    archive = args.archive.read_bytes() if args.archive else subprocess.check_output(
        ["git", "-C", str(args.repo), "archive", "--format=tar.gz", BASE]
    )
    report["base_archive_sha256"] = sha(archive)
    try:
        for variant, changes in variants.items():
            state("building " + variant)
            with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
                tar.extractall(source, filter="data")
            for name in changes:
                subprocess.run(["git", "apply", "-"], input=patches[name], text=True, cwd=source, check=True)
            with (args.work / (variant + "-build.log")).open("w") as log:
                subprocess.run(["cargo", "build", "--release", "--locked", "-p", "rust-markdownlint-cli",
                                "--target", TARGET, "--target-dir", str(args.repo / "target")],
                               cwd=source, stdout=log, stderr=subprocess.STDOUT, check=True)
            binary = bins / variant
            shutil.copy2(args.repo / "target" / TARGET / "release/rust-markdownlint", binary)
            report["binaries"][variant] = {"sha256": sha(binary.read_bytes()), "bytes": binary.stat().st_size}
        orders = list(itertools.permutations(variants))
        for name in ["fixtures", "blog", "fixtures-10x"]:
            state("validating " + name)
            corpus = args.corpora / name
            digest = hashlib.sha256()
            files = sorted(corpus.rglob("*.md"))
            if name == "fixtures-10x":
                files.sort(key=lambda p: (int(p.relative_to(corpus).parts[0]), p.name))
            for path in files:
                relative = path.relative_to(corpus).as_posix().encode()
                content = path.read_bytes()
                digest.update(len(relative).to_bytes(8, "big") + relative)
                digest.update(len(content).to_bytes(8, "big") + content)
            entry = {"name": name, "files": len(files), "sha256": digest.hexdigest(),
                     "validation": {}, "results": {v: {"seconds": []} for v in variants}, "orders": []}
            report["corpora"].append(entry)
            commands = {v: [str(bins / v), "**/*.md"] for v in variants}
            baseline = None
            for variant, command in commands.items():
                result = subprocess.run(command, cwd=corpus, capture_output=True, timeout=300)
                value = (result.returncode, result.stdout, result.stderr)
                if baseline is None:
                    baseline = value
                assert value == baseline and result.returncode == 1, (name, variant, "output mismatch")
                entry["validation"][variant] = {"exit_code": result.returncode,
                    "stdout_sha256": sha(result.stdout), "stderr_sha256": sha(result.stderr),
                    "stderr_bytes": len(result.stderr), "matches_baseline": True}
            for _ in range(3):
                for command in commands.values():
                    result = subprocess.run(command, cwd=corpus, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    assert result.returncode == baseline[0]
            for i, order in enumerate(orders):
                entry["orders"].append(order)
                for variant in order:
                    start = time.perf_counter()
                    result = subprocess.run(commands[variant], cwd=corpus, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    elapsed = time.perf_counter() - start
                    assert result.returncode == baseline[0]
                    entry["results"][variant]["seconds"].append(elapsed)
                state(f"measuring {name} {i + 1}/24")
                save()
            base_samples = entry["results"]["baseline"]["seconds"]
            for variant, result in entry["results"].items():
                samples = result["seconds"]
                paired = [a - b for a, b in zip(base_samples, samples)]
                result.update(mean=statistics.mean(samples), stddev=statistics.stdev(samples), median=statistics.median(samples),
                              paired_savings_mean=statistics.mean(paired), paired_savings_median=statistics.median(paired),
                              faster_rounds=sum(x > 0 for x in paired))
                print(name, variant, result["mean"] * 1000, result["stddev"] * 1000, flush=True)
            save()
        state("complete")
    except BaseException:
        state("failed")
        raise


if __name__ == "__main__":
    main()
