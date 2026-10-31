#!/usr/bin/env python3
"""Capture the existing bootstrap measurement; profiling is explicitly opt-in."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from datetime import datetime, timezone


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", action="store_true")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    args.output.mkdir(parents=True, exist_ok=False)
    if not args.skip_build:
        subprocess.run(["npm", "run", "build:commit-perf" if args.profile else "build:happ", "-w", "map-happ"], check=True)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    artifact = Path("happ/workdir/map_holons.dna")
    metadata = {"revision": revision, "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                "profile": args.profile, "instrumentation": "822-v2" if args.profile else "unprofiled",
                "devdocs_revision": subprocess.check_output(["git", "-C", "../map-dev-docs", "rev-parse", "HEAD"], text=True).strip(),
                "artifact_sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
                "runs": args.runs, "workload": "generated-bootstrap", "substrate": "loader-commits",
                "host_snapshot": "only if commit_internal is exercised; loader observations remain separate"}
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    (args.output / "working-tree.patch").write_bytes(subprocess.check_output(["git", "diff", "HEAD"]))
    env = {**os.environ, "RUST_LOG": "warn,holons_test=info,bootstrap_performance_tests=info,holons_client=info,holons_guest=info,wasm=info", "WASM_LOG": "info"}
    if args.profile:
        env["MAP_PROFILE"] = "1"
    else:
        env.pop("MAP_PROFILE", None)
    observations = []
    for run in range(args.runs):
        path = args.output / f"run-{run+1:02}.log"
        with path.open("w") as log:
            subprocess.run(["cargo", "test", "--manifest-path", "tests/sweetests/Cargo.toml", "--test", "bootstrap_performance_tests", "measures_generated_core_schema_bootstrap", "--", "--exact", "--ignored", "--nocapture", "--test-threads=1"], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        contents = path.read_text()
        totals = re.findall(r"sweettest_bootstrap_measurement: total_ms=(\d+)", contents)
        if len(totals) != 1 or (args.profile and "[commit-derivation]" not in contents):
            raise ValueError(f"Incomplete bootstrap observations: {path}")
        observations.append({"run": run+1, "total_ms": int(totals[0])})
    (args.output / "measurements.json").write_text(json.dumps(observations, indent=2) + "\n")


if __name__ == "__main__":
    main()
