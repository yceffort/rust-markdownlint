#!/usr/bin/env python3
"""Compare three CLIs on isolated corpora; keep samples and output checks as JSON."""

import argparse
import datetime
import hashlib
import itertools
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tempfile
import time


def command_output(*command):
    return subprocess.check_output(command, text=True).strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(command, cwd, capture=False):
    start = time.perf_counter()
    result = subprocess.run(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
        stderr=subprocess.PIPE if capture else subprocess.DEVNULL,
        # A timeout makes Python poll waitpid with sleeps (up to 50 ms on Linux).
        # Use blocking wait for timings so short-lived tools are not overcounted.
        timeout=300 if capture else None,
    )
    elapsed = time.perf_counter() - start
    if result.returncode not in (0, 1):
        raise RuntimeError(f"Unexpected exit {result.returncode}: {command}")
    return result, elapsed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", type=Path, required=True)
    parser.add_argument("--rumdl", type=Path, required=True)
    parser.add_argument("--cli2", type=Path, required=True)
    parser.add_argument("--compatible-cli2", type=Path, required=True)
    parser.add_argument("--blog", type=Path, required=True, help="Pinned blog checkout")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--warmup", type=int, default=3)
    args = parser.parse_args()
    if args.runs < 2 or args.warmup < 0:
        parser.error("--runs must be at least 2; --warmup must be nonnegative")

    repo = Path(__file__).resolve().parent.parent
    tools = {
        name: str(getattr(args, name).resolve())
        for name in ("rust", "rumdl", "cli2")
    }
    compatible = str(args.compatible_cli2.resolve())
    blog = args.blog.resolve()
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)

    def cli2_version(binary):
        for directory in Path(binary).resolve().parents:
            manifest = directory / "package.json"
            if manifest.exists():
                return json.loads(manifest.read_text())["version"]
        raise RuntimeError(f"No package.json above {binary}")

    report = {
        "date_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "source_commit": command_output("git", "-C", str(repo), "rev-parse", "HEAD"),
        "source_status": command_output("git", "-C", str(repo), "status", "--porcelain"),
        "rust_binary_sha256": digest(Path(tools["rust"]).read_bytes()),
        "blog_commit": command_output("git", "-C", str(blog), "rev-parse", "HEAD"),
        "environment": {
            "platform": platform.platform(),
            "os_release": Path("/etc/os-release").read_text(),
            "lscpu": command_output("lscpu"),
            "memory": command_output("free", "-b"),
            "logical_cpus": os.cpu_count(),
            "rustc": command_output("rustc", "--version"),
            "cargo": command_output("cargo", "--version"),
            "node": command_output("node", "--version"),
            "rumdl": command_output(tools["rumdl"], "--version"),
            "cli2": cli2_version(tools["cli2"]),
            "compatible_cli2": cli2_version(compatible),
            "rayon_num_threads": os.environ.get("RAYON_NUM_THREADS"),
        },
        "method": {
            "runs": args.runs,
            "warmup": args.warmup,
            "timing": "perf_counter wall seconds around subprocess.run with blocking wait; includes process startup",
            "order": "Cycle through all six permutations of the three tools, one sample each per round",
            "output": "Default formatters run; stdout/stderr redirected to /dev/null",
            "cache": "Warm filesystem cache; rumdl persistent cache disabled with --no-cache",
            "configuration": "Markdown files only, with noBanner:true; default rule sets and inline directives",
        },
        "corpora": [],
    }
    fixtures = repo / "crates/core/tests/fixtures/markdownlint"
    posts = blog / "apps/blog/posts"
    cases = [("fixtures", fixtures, 1), ("blog", posts, 1), ("fixtures-10x", fixtures, 10)]
    orders = list(itertools.permutations(tools))

    def save():
        output.write_text(json.dumps(report, indent=2) + "\n")

    # Keep inputs outside repository configuration discovery, and remove only our temporary files.
    with tempfile.TemporaryDirectory(prefix="markdownlint-tools-") as temporary:
        for name, source, scale in cases:
            corpus = Path(temporary) / name
            corpus.mkdir()
            candidates = source.rglob("*.md") if name == "blog" else source.glob("*.md")
            files = sorted(candidates)
            if not files:
                raise RuntimeError(f"Empty corpus: {source}")
            corpus_hash = hashlib.sha256()
            byte_count = 0
            for copy in range(1, scale + 1):
                for original in files:
                    relative = original.relative_to(source)
                    if scale > 1:
                        relative = Path(str(copy)) / relative
                    target = corpus / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    content = original.read_bytes()
                    target.write_bytes(content)
                    relative_bytes = relative.as_posix().encode()
                    corpus_hash.update(len(relative_bytes).to_bytes(8, "big") + relative_bytes)
                    corpus_hash.update(len(content).to_bytes(8, "big") + content)
                    byte_count += len(content)
            (corpus / ".markdownlint-cli2.jsonc").write_text('{"noBanner":true}\n')
            commands = {
                "rust": [tools["rust"], "**/*.md"],
                "rumdl": [tools["rumdl"], "check", "--no-cache", "--no-config", "."],
                "cli2": [tools["cli2"], "**/*.md"],
            }
            entry = {
                "name": name,
                "files": len(files) * scale,
                "bytes": byte_count,
                "sha256": corpus_hash.hexdigest(),
                "commands": commands,
                "validation": {},
                "results": {tool: {"seconds": []} for tool in tools},
            }
            report["corpora"].append(entry)
            captured = {}
            validation_commands = {
                **commands, "compatible_cli2": [compatible, "**/*.md"]
            }
            for tool, command in validation_commands.items():
                result, _ = run(command, corpus, capture=True)
                captured[tool] = result
                entry["validation"][tool] = {
                    "exit_code": result.returncode,
                    "stdout_sha256": digest(result.stdout),
                    "stderr_sha256": digest(result.stderr),
                    "stdout_bytes": len(result.stdout),
                    "stderr_bytes": len(result.stderr),
                    "stderr_lines": len(result.stderr.splitlines()),
                }
            for tool in ("cli2", "compatible_cli2"):
                entry["validation"][f"rust_matches_{tool}"] = all(
                    getattr(captured["rust"], key) == getattr(captured[tool], key)
                    for key in ("returncode", "stdout", "stderr")
                )
            rumdl_json, _ = run(
                commands["rumdl"] + ["--output-format", "json"], corpus, capture=True
            )
            entry["validation"]["rumdl"]["diagnostics"] = len(
                json.loads(rumdl_json.stdout)
            )
            print(
                f"{name}: {entry['files']} files, {byte_count} bytes; "
                f"validation {entry['validation']}", flush=True
            )
            for _ in range(args.warmup):
                for command in commands.values():
                    run(command, corpus)
            for round_index in range(args.runs):
                for tool in orders[round_index % len(orders)]:
                    _, elapsed = run(commands[tool], corpus)
                    entry["results"][tool]["seconds"].append(elapsed)
                save()
                print(f"{name}: round {round_index + 1}/{args.runs}", flush=True)
            for tool, result in entry["results"].items():
                samples = result["seconds"]
                result.update(
                    mean=statistics.mean(samples),
                    stddev=statistics.stdev(samples),
                    median=statistics.median(samples),
                    minimum=min(samples),
                    maximum=max(samples),
                )
                print(
                    f"{name} {tool}: {result['mean'] * 1000:.1f} "
                    f"± {result['stddev'] * 1000:.1f} ms", flush=True
                )
            save()


if __name__ == "__main__":
    main()
