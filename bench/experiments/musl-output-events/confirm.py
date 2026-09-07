#!/usr/bin/env python3
"""Confirm existing binaries with shuffled balanced orders and child CPU time."""

import argparse
import itertools
import json
from pathlib import Path
import random
import resource
import statistics
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--corpora", type=Path, required=True)
    args = parser.parse_args()
    report = json.loads((args.work / "results.json").read_text())
    assert (args.work / "status").read_text().strip() == "complete"
    report["confirmation"] = {"seed": 208, "warmup": 3, "runs": 24,
        "order": "all 24 permutations, shuffled with seed 208",
        "cpu": "difference in RUSAGE_CHILDREN user + system time outside the wall timer", "corpora": []}
    variants = list(report["binaries"])
    orders = list(itertools.permutations(variants))
    random.Random(208).shuffle(orders)

    def save():
        (args.work / "confirmed.json").write_text(json.dumps(report, indent=2) + "\n")

    def run(variant, cwd):
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        start = time.perf_counter()
        result = subprocess.run([str(args.work / "bin" / variant), "**/*.md"], cwd=cwd,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        wall = time.perf_counter() - start
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        assert result.returncode == 1
        return wall, after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime

    for original in report["corpora"]:
        name = original["name"]
        cwd = args.corpora / name
        entry = {"name": name, "sha256": original["sha256"], "orders": orders,
                 "results": {v: {"seconds": [], "cpu_seconds": []} for v in variants}}
        report["confirmation"]["corpora"].append(entry)
        for _ in range(3):
            for variant in variants:
                run(variant, cwd)
        for i, order in enumerate(orders):
            for variant in order:
                wall, cpu = run(variant, cwd)
                entry["results"][variant]["seconds"].append(wall)
                entry["results"][variant]["cpu_seconds"].append(cpu)
            print(name, i + 1, flush=True)
            save()
        baseline = entry["results"]["baseline"]["seconds"]
        for variant, result in entry["results"].items():
            samples = result["seconds"]
            paired = [a - b for a, b in zip(baseline, samples)]
            result.update(mean=statistics.mean(samples), stddev=statistics.stdev(samples),
                          median=statistics.median(samples), cpu_mean=statistics.mean(result["cpu_seconds"]),
                          paired_savings_mean=statistics.mean(paired), paired_savings_median=statistics.median(paired),
                          faster_rounds=sum(x > 0 for x in paired))
            print(name, variant, result["mean"] * 1000, result["stddev"] * 1000, "cpu", result["cpu_mean"] * 1000, flush=True)
        save()
    (args.work / "confirm-status").write_text("complete\n")


if __name__ == "__main__":
    main()
