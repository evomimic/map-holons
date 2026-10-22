#!/usr/bin/env python3
"""Capture isolated real Commit runs and summarize aggregate measurements."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
from datetime import datetime, timezone


def output(*command):
    return subprocess.check_output(command, text=True).strip()


def summarize(logs, expected_samples=6, variant="prefetch", assessment="standard", mode="create"):
    if not logs:
        raise ValueError("No captured run logs were found")
    samples = []
    ansi = re.compile(r"\x1b\[[0-9;]*m")
    for log in logs:
        sample = None
        for raw in log.read_text().splitlines():
            line = ansi.sub("", raw)
            fields = dict(re.findall(r"([a-zA-Z_0-9]+)=([^\s]+)", line))
            if "[commit-baseline]" in line and "sample_start" in fields:
                if sample is not None:
                    raise ValueError(f"Unfinished sample in {log}")
                sample = {"run": log.name, "condition": fields["condition"], "sample": int(fields["sample_start"]), "metrics": {}}
            if sample is None:
                continue
            tag = re.search(r"\[(commit-[a-z-]+)\]", line)
            if tag:
                prefix = tag[1]
                if prefix == "commit-read-cost":
                    prefix += "." + fields["phase"] + "." + fields["component"]
                elif prefix == "commit-assessment":
                    prefix += "." + fields["phase"]
                elif prefix == "commit-preparation-type":
                    prefix += "." + fields["type"]
                elif prefix == "commit-preparation-operation":
                    prefix += "." + fields["operation"] + "." + fields["role"]
                elif prefix == "commit-preparation-edge":
                    prefix += "." + fields["role"] + "." + fields["edge"]
            elif "[PERF-688] guest_commit:" in line:
                prefix = "guest_commit"
            elif "[MAP-PROFILE]" in line and fields.get("command") == "commit":
                prefix = "host_command"
            else:
                continue
            for key, value in fields.items():
                if re.fullmatch(r"-?\d+(\.\d+)?", value):
                    sample["metrics"][prefix + "." + key] = float(value)
            if "sample_end" in fields:
                if int(fields["sample_end"]) != sample["sample"]:
                    raise ValueError(f"Mismatched sample in {log}")
                required = ["commit-baseline.command_us", "commit-adapter.total_us", "commit-transport.conductor_call_us", "commit-construction.elapsed_us", "commit-preparation.distinct_content", "guest_commit.validation_ms", "commit-preparation-operation.EffectiveContract.PreparedDefinition.elapsed_us", "commit-preparation-operation.ContentRetention.SavedContent.exact_node_reads"]
                if variant == "prefetch":
                    required.append("commit-preparation-operation.MembershipPrefetch." + ("ValidatorRoot" if assessment in ["validator-package", "combined", "warm-probe"] else "Commitment") + ".elapsed_us")
                elif any(key.startswith("commit-preparation-operation.MembershipPrefetch.") for key in sample["metrics"]):
                    raise ValueError(f"On-demand capture unexpectedly contains eager prefetch in {log}")
                missing = [key for key in required if key not in sample["metrics"]]
                if missing:
                    raise ValueError(f"Missing measurements in {log}: {missing}")
                experiment_metric = "commit-preparation-operation.RelationshipDerivedInputs.InstanceExperiment.elapsed_us"
                if (experiment_metric in sample["metrics"]) != (assessment in ["instance-experiment", "combined", "warm-probe"]):
                    raise ValueError(f"Guest assessment path does not match {assessment} in {log}")
                package_metric = "commit-preparation-operation.MembershipPrefetch.ValidatorRoot.elapsed_us"
                if (package_metric in sample["metrics"]) != (assessment in ["validator-package", "combined", "warm-probe"]):
                    raise ValueError(f"Guest package path does not match {assessment} in {log}")
                warm_metric = "commit-preparation-operation.WarmValidatorAssessment.SameTransactionProbe.invocations"
                if (warm_metric in sample["metrics"]) != (assessment == "warm-probe"):
                    raise ValueError(f"Guest warm probe does not match {assessment} in {log}")
                if assessment == "warm-probe" and sample["metrics"][warm_metric] != 3:
                    raise ValueError(f"Expected three warm assessments in {log}")
                # Anchor lineage evidence is loaded once per assessment, and only for version-producing updates.
                # Requiring it per mode confirms each sample exercised the path the mode is meant to measure.
                anchor_metric = "commit-preparation-operation.saved_lineage_roots_by_key.anchor-evidence.invocations"
                if sample["metrics"].get(anchor_metric, 0) != (1 if mode == "new-version" else 0):
                    raise ValueError(f"Anchor evidence does not match {mode} mode in {log}")
                costs = [value for key, value in sample["metrics"].items()
                         if key.startswith("commit-read-cost.Construction.") and key.endswith(".elapsed_us")]
                if costs:
                    metrics = sample["metrics"]
                    accounted = metrics.get("commit-cost-accounting.accounted_us")
                    wall = metrics.get("commit-cost-accounting.construction_us")
                    gap = metrics.get("commit-cost-accounting.timer_gap_us")
                    if accounted is None or wall is None or gap is None or sum(costs) != accounted or wall - accounted != gap:
                        raise ValueError(f"Inconsistent exclusive construction accounting in {log}")
                samples.append(sample)
                sample = None
        if sample is not None:
            raise ValueError(f"Unfinished sample in {log}")
    if len(samples) != len(logs) * expected_samples:
        raise ValueError(f"Expected {expected_samples} complete samples per run, got {len(samples)}")
    summary = {}
    for condition in ["first", "repeated"]:
        selected = [sample for sample in samples if sample["condition"] == condition]
        summary[condition] = {}
        for metric in sorted({key for sample in selected for key in sample["metrics"]}):
            values = [sample["metrics"][metric] for sample in selected if metric in sample["metrics"]]
            summary[condition][metric] = {"n": len(values), "median": statistics.median(values), "min": min(values), "max": max(values), "stdev": statistics.stdev(values) if len(values) > 1 else None}
    return {"variant": variant, "assessment": assessment, "mode": mode, "samples": samples, "summary": summary}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--samples", type=int, default=6)
    parser.add_argument("--subjects", type=int, default=1)
    parser.add_argument("--mode", choices=["create", "new-version"], default="create", help="create: new instances per Commit; new-version: one property-only successor of a saved non-root version per Commit")
    parser.add_argument("--variant", choices=["prefetch", "on-demand"], default="prefetch")
    parser.add_argument("--readiness-cases", action="store_true")
    parser.add_argument("--assessment", choices=["standard", "instance-experiment", "validator-package", "combined", "warm-probe"], default="standard", help="Label the separately built guest artifact; this does not select or build it")
    parser.add_argument("--index-mode", choices=["none", "control", "base-hash"], default="none")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--summarize-only", action="store_true")
    args = parser.parse_args()
    if args.runs < 1 or args.samples < 2 or args.subjects < 1:
        parser.error("--runs must be positive and --samples must be at least two")
    if args.mode == "new-version" and (args.subjects != 1 or args.readiness_cases):
        parser.error("--mode new-version measures one subject per Commit without readiness cases")
    if args.summarize_only:
        result = summarize(sorted(args.output.glob("run-*.log")), args.samples, args.variant, args.assessment, args.mode)
    else:
        args.output.mkdir(parents=True, exist_ok=False)
        env = {**os.environ, "MAP_COMMIT_PERF_SAMPLES": str(args.samples), "MAP_COMMIT_PERF_SUBJECTS": str(args.subjects), "MAP_PROFILE": "1", "RUST_LOG": "warn,holons_test=info,commit_performance_tests=info,holons_guest=info,wasm=info", "WASM_LOG": "info"}
        env["MAP_COMMIT_PERF_INDEX_MODE"] = args.index_mode
        env["MAP_COMMIT_PERF_MODE"] = args.mode
        paths = [Path("happ/workdir/map_holons.dna"), Path("happ/Cargo.lock"), Path("host/Cargo.lock"), Path("Cargo.lock"), Path("flake.lock")]
        paths += sorted(Path("generated/core-schema-bootstrap").rglob("*.json"))
        paths += [Path(path) for path in output("git", "ls-files", "--modified", "--others", "--exclude-standard").splitlines() if Path(path).is_file()]
        paths = sorted(set(paths))
        metadata = {
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "revision": output("git", "rev-parse", "HEAD"),
            "branch": output("git", "branch", "--show-current"),
            "status": output("git", "status", "--short"),
            "platform": platform.platform(), "machine": platform.machine(), "cpu_count": os.cpu_count(),
            "devdocs_revision": output("git", "-C", "../map-dev-docs", "rev-parse", "HEAD"),
            "rustc": output("rustc", "--version"), "cargo": output("cargo", "--version"),
            "runs": args.runs, "samples_per_run": args.samples, "subjects_per_commit": args.subjects, "mode": args.mode, "variant": args.variant, "index_mode": args.index_mode, "assessment": args.assessment,
            "environment": {key: env[key] for key in ["MAP_PROFILE", "RUST_LOG", "WASM_LOG", "MAP_COMMIT_PERF_SAMPLES", "MAP_COMMIT_PERF_SUBJECTS", "MAP_COMMIT_PERF_MODE"]},
            "sha256": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths},
        }
        (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
        (args.output / "working-tree.patch").write_text(output("git", "diff", "HEAD") + "\n")
        logs = []
        for run in range(args.runs):
            path = args.output / f"run-{run + 1:02}.log"
            print(f"Commit baseline run {run + 1}/{args.runs}: {path}", flush=True)
            run_env = dict(env)
            if args.readiness_cases:
                run_env["MAP_COMMIT_READINESS_CASES_OUTPUT"] = str((args.output / f"run-{run + 1:02}-cases.json").resolve())
            if args.index_mode != "none":
                run_env["MAP_COMMIT_PERF_INDEX_OUTPUT"] = str((args.output / f"run-{run + 1:02}-index.json").resolve())
            with path.open("w") as log:
                result = subprocess.run(["cargo", "test", "--manifest-path", "tests/sweetests/Cargo.toml", "--test", "commit_performance_tests", "--", "--nocapture", "--test-threads=1"], stdout=log, stderr=subprocess.STDOUT, env=run_env)
            if result.returncode:
                print(f"Run failed; inspect {path}", file=sys.stderr)
                return result.returncode
            logs.append(path)
        result = summarize(logs, args.samples, args.variant, args.assessment, args.mode)
    (args.output / "measurements.json").write_text(json.dumps(result, indent=2) + "\n")
    for condition, metrics in result["summary"].items():
        value = metrics["commit-baseline.command_us"]
        print(f"{condition}: n={value['n']} median={value['median']/1000:.3f} ms range={value['min']/1000:.3f}–{value['max']/1000:.3f} ms")
    print(f"Full measurements: {args.output / 'measurements.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
