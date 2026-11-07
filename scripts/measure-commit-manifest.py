#!/usr/bin/env python3
"""Run the bounded Commit manifest after building the profiling artifact once."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--manifest", type=Path, default=Path("scripts/commit-performance-manifest.json"))
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--smoke", action="store_true", help="One run and two samples per workload, one bootstrap run")
parser.add_argument("--skip-build", action="store_true")
parser.add_argument("--start-at", help="Resume at a named workload (or bootstrap), writing a new output directory")
parser.add_argument("--exclude", action="append", default=[], help="Omit a named workload; repeatable and recorded in the output manifest")
parser.add_argument("--runs", type=int, help="Override the manifest run count")
parser.add_argument("--samples", type=int, help="Override the manifest sample count")
parser.add_argument("--bootstrap-runs", type=int, help="Override the manifest bootstrap run count")
args = parser.parse_args()
manifest = json.loads(args.manifest.read_text())
names = [workload["name"] for workload in manifest["workloads"]] + ["bootstrap"]
unknown = [name for name in args.exclude if name not in names[:-1]]
if unknown:
    parser.error(f"Unknown --exclude workload: {', '.join(unknown)}")
runs = args.runs or (1 if args.smoke else manifest["runs"])
samples = args.samples or (2 if args.smoke else manifest["samples"])
bootstrap_runs = args.bootstrap_runs or (1 if args.smoke else manifest["bootstrap"]["runs"])
if args.start_at and args.start_at not in names:
    parser.error(f"Unknown --start-at workload: {args.start_at}")
if args.start_at and not args.smoke:
    parser.error("--start-at is limited to smoke runs; baseline captures require the complete manifest")
if not args.skip_build:
    subprocess.run(["npm", "run", "build:commit-perf", "-w", "map-happ"], check=True)
revision = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
if not args.smoke and subprocess.check_output(["git", "status", "--porcelain"], text=True).strip():
    parser.error("Commit workload/instrumentation changes before baseline capture")
args.output.mkdir(parents=True, exist_ok=False)
(args.output / "manifest.json").write_text(json.dumps({**manifest, "workload_revision": revision, "smoke": args.smoke, "start_at": args.start_at,
                                                       "excluded": args.exclude, "captured_runs": runs, "captured_samples": samples,
                                                       "captured_bootstrap_runs": bootstrap_runs}, indent=2) + "\n")
for index, workload in enumerate(manifest["workloads"]):
    if (args.start_at and index < names.index(args.start_at)) or workload["name"] in args.exclude:
        continue
    result = subprocess.run([sys.executable, "scripts/measure-commit-performance.py", *workload["args"], "--runs", str(runs), "--samples", str(samples),
                             "--guest-artifact-revision", revision, "--semantic-revision", revision, "--output", str(args.output / workload["name"])])
    if result.returncode:
        print(f"Smoke/capture stopped at {workload['name']}; inspect {args.output / workload['name']}", file=sys.stderr)
        raise SystemExit(result.returncode)
subprocess.run([sys.executable, "scripts/measure-bootstrap-performance.py", "--profile", "--skip-build", "--runs", str(bootstrap_runs), "--output", str(args.output / "bootstrap")], check=True)
