#!/usr/bin/env python3
"""Capture forced retrieval strategies on identical live-conductor workloads."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys


def summarize(captures):
    groups = {}
    for capture in captures:
        rows = json.loads(capture.read_text())
        if not rows:
            raise ValueError(f"Empty capture: {capture}")
        for row in rows:
            key = tuple(row[field] for field in ("source_links", "distribution", "requested", "start", "strategy"))
            groups.setdefault(key, []).append(row)
    summary = []
    for key, rows in sorted(groups.items()):
        item = dict(zip(("source_links", "distribution", "requested", "start", "strategy"), key))
        item.update(samples=len(rows), selected_links=rows[0]["selected_links"])
        for metric in ("guest_us", "roundtrip_us"):
            values = [row[metric] for row in rows]
            item[metric] = {"median": statistics.median(values), "min": min(values), "max": max(values),
                            "stdev": statistics.stdev(values) if len(values) > 1 else 0}
        for metric in ("backend_calls", "queries", "returned_links"):
            values = {row[metric] for row in rows}
            if len(values) != 1:
                raise ValueError(f"Inconsistent {metric}: {key}")
            item[metric] = values.pop()
        summary.append(item)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--samples", type=int, default=6)
    parser.add_argument("--skip-build", action="store_true", help="use already rebuilt and audited artifacts")
    args = parser.parse_args()
    if args.runs < 1 or not 3 <= args.samples <= 60:
        parser.error("runs must be positive; samples must be 3..60")
    root = Path(__file__).resolve().parent.parent
    os.chdir(root)
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    if any(args.output.iterdir()):
        parser.error("use an empty output directory to preserve prior evidence")
    if not args.skip_build:
        for command in (["npm", "run", "build:happ"], ["npm", "run", "build:probes", "-w", "map-happ"],
                        ["npm", "run", "check:happ-artifacts"]):
            subprocess.run(command, check=True)
    metadata = {"platform": platform.platform(), "samples": args.samples, "runs": args.runs,
                "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
                "rust": subprocess.check_output(["rustc", "--version"], text=True).strip(),
                "artifacts": {}}
    for filename in ("happ/workdir/map_holons.dna", "happ/target/wasm32-unknown-unknown/release/holons_test_probes.wasm"):
        metadata["artifacts"][filename] = hashlib.sha256(Path(filename).read_bytes()).hexdigest()
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2))
    (args.output / "working-tree.patch").write_bytes(subprocess.check_output(["git", "diff", "HEAD"]))
    # Include untracked benchmark sources in the evidence fingerprint as well.
    source_files = [Path(__file__).resolve(), root / "tests/sweetests/tests/relationship_retrieval_performance_tests.rs"]
    metadata["benchmark_sources"] = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_files}
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2))
    captures = []
    for run in range(1, args.runs + 1):
        capture = args.output / f"run-{run:02}.json"
        environment = dict(os.environ, RUST_LOG="warn", WASM_LOG="warn",
                           MAP_RELATIONSHIP_PERF_SAMPLES=str(args.samples), MAP_RELATIONSHIP_PERF_OUTPUT=str(capture))
        print(f"Retrieval benchmark process {run}/{args.runs}", flush=True)
        with (args.output / f"run-{run:02}.log").open("w") as log:
            result = subprocess.run(["cargo", "test", "--manifest-path", "tests/sweetests/Cargo.toml",
                "--test", "relationship_retrieval_performance_tests", "--", "--ignored", "--nocapture"],
                env=environment, stdout=log, stderr=subprocess.STDOUT)
        if result.returncode:
            print(f"Benchmark failed; inspect {log.name}", file=sys.stderr)
            return result.returncode
        rows = json.loads(capture.read_text())
        if len(rows) != 6 * 5 * 3 * args.samples * 3:
            raise ValueError("Incomplete workload matrix")
        captures.append(capture)
    summary = summarize(captures)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2))
    comparisons = {}
    for item in summary:
        key = tuple(item[field] for field in ("source_links", "distribution", "requested", "start"))
        comparisons.setdefault(key, {})[item["strategy"]] = item
    with (args.output / "comparison.csv").open("w", newline="") as stream:
        fields = ["source_links", "distribution", "requested", "start", "selected_links",
                  "separate_prefixes_us", "batched_prefixes_us", "consolidated_us",
                  "consolidated_over_batched", "fastest"]
        writer = csv.DictWriter(stream, fieldnames=fields)
        writer.writeheader()
        for key, strategies in sorted(comparisons.items()):
            row = dict(zip(fields[:4], key))
            row["selected_links"] = strategies["consolidated"]["selected_links"]
            for strategy, item in strategies.items():
                row[strategy + "_us"] = item["guest_us"]["median"]
            row["consolidated_over_batched"] = row["consolidated_us"] / max(1, row["batched_prefixes_us"])
            row["fastest"] = min(strategies, key=lambda strategy: row[strategy + "_us"])
            writer.writerow(row)
    print(f"Results: {args.output / 'comparison.csv'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
