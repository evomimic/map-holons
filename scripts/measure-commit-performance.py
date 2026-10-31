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


ASSESSMENT_PHASES = ["Construction", "SubjectPreparation", "SchemaWorksetPreparation",
                     "PackageDiscovery", "PreparedProducts", "PackageValidation", "InstanceValidation"]
DERIVATION_OPERATIONS = ["schema_resolution_requests", "schema_builder_attempts", "schema_builder_successes",
                         "saved_restoration_attempts", "saved_restoration_successes",
                         "contract_contribution_resolutions", "constraint_contribution_resolutions",
                         "structural_diagnoses", "role_walks", "snapshot_lookups", "registry_entries_examined",
                         "registry_hits", "name_index_builds"]

def output(*command):
    return subprocess.check_output(command, text=True).strip()


def summarize(logs, expected_samples=6, variant="prefetch", assessment="standard", mode="create",
              anchor_evidence_policy="per-new-version", instrumentation="822-v2",
              preparation_substrate="broad-readiness", host_snapshot_policy="optional"):
    if not logs:
        raise ValueError("No captured run logs were found")
    if anchor_evidence_policy not in ["per-new-version", "retired"]:
        raise ValueError(f"Unknown anchor evidence policy: {anchor_evidence_policy}")
    if instrumentation not in ["historical", "822-v2"]:
        raise ValueError(f"Unknown instrumentation: {instrumentation}")
    if preparation_substrate not in ["broad-readiness", "declared-demand"]:
        raise ValueError(f"Unknown preparation substrate: {preparation_substrate}")
    if host_snapshot_policy not in ["optional", "required"]:
        raise ValueError(f"Unknown host snapshot policy: {host_snapshot_policy}")
    samples = []
    ansi = re.compile(r"\x1b\[[0-9;]*m")
    for log in logs:
        sample = None
        run_samples = 0
        for raw in log.read_text().splitlines():
            line = ansi.sub("", raw)
            fields = dict(re.findall(r"([a-zA-Z_0-9]+)=([^\s]+)", line))
            if "[commit-baseline]" in line and "sample_start" in fields:
                if sample is not None:
                    raise ValueError(f"Unfinished sample in {log}")
                sample = {"run": log.name, "condition": fields["condition"], "sample": int(fields["sample_start"]), "metrics": {}, "host_snapshot": "unavailable", "host_shipping": "unavailable"}
            if sample is None:
                continue
            tag = re.search(r"\[(commit-[a-z-]+)\]", line)
            if tag:
                prefix = tag[1]
                if prefix == "commit-instrumentation":
                    if fields.get("version") != instrumentation or fields.get("substrate") != preparation_substrate:
                        raise ValueError(f"Incompatible instrumentation or substrate in {log}")
                    if fields.get("read_detail") != "disabled":
                        raise ValueError(f"Timed captures require read_detail=disabled in {log}")
                    sample["instrumentation"] = fields["version"]
                if prefix == "commit-host-shipping":
                    sample["host_shipping"] = "measured-envelope"
                if prefix == "commit-host-snapshot":
                    sample["host_snapshot"] = "measured"
                if prefix in ["commit-logical-reads", "commit-derivation"]:
                    prefix += "." + fields["phase"]

                elif prefix == "commit-logical-read":
                    # Inventory evidence stays in raw logs, never in performance metric keys.
                    continue
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
                    metric = prefix + "." + key
                    # Multiple aggregate edge records may be emitted; never overwrite demand.
                    if tag and tag[1] in ["commit-preparation-edge"]:
                        sample["metrics"][metric] = sample["metrics"].get(metric, 0) + float(value)
                    else:
                        sample["metrics"][metric] = float(value)
            if "sample_end" in fields:
                if int(fields["sample_end"]) != sample["sample"]:
                    raise ValueError(f"Mismatched sample in {log}")
                required = ["commit-baseline.command_us", "commit-adapter.total_us", "commit-transport.conductor_call_us", "commit-construction.elapsed_us", "commit-preparation.distinct_content", "guest_commit.validation_ms", "commit-preparation-operation.EffectiveContract.PreparedDefinition.elapsed_us", "commit-preparation-operation.ContentRetention.SavedContent.exact_node_reads"]
                if instrumentation == "822-v2":
                    if sample.get("instrumentation") != instrumentation:
                        raise ValueError(f"Missing instrumentation marker in {log}")
                    required.remove("commit-preparation-operation.EffectiveContract.PreparedDefinition.elapsed_us")
                    required += ["commit-preparation.requested_schema_products", "commit-preparation.constructed_schema_products"]
                    for phase in ASSESSMENT_PHASES:
                        required += [f"commit-logical-reads.{phase}.requests", f"commit-logical-reads.{phase}.distinct_memberships"]
                        required += [f"commit-logical-reads.{phase}.{field}" for field in ["membership_requests", "content_requests", "hits", "misses"]]
                        required += [f"commit-derivation.{phase}.{operation}" for operation in DERIVATION_OPERATIONS]
                    if preparation_substrate == "broad-readiness":
                        required += [f"commit-preparation-operation.{operation}.Assessment.elapsed_us"
                                     for operation in ["ReadinessDiscovery", "DependencyGrouping"]]
                        required.append("commit-preparation-edge.Readiness.Extends.requests")
                    elif any(key.startswith("commit-preparation-operation.ReadinessDiscovery.") for key in sample["metrics"]):
                        raise ValueError(f"Final substrate contains broad readiness discovery in {log}")
                if host_snapshot_policy == "required":
                    required += [f"commit-host-snapshot.{field}" for field in ["construction_us", "entries", "shipped_holons"]]
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
                # Read the capture's policy so archived baselines remain interpretable after retirement.
                anchor_metric = "commit-preparation-operation.saved_lineage_roots_by_key.anchor-evidence.invocations"
                expected_anchor_loads = int(anchor_evidence_policy == "per-new-version" and mode == "new-version")
                if sample["metrics"].get(anchor_metric, 0) != expected_anchor_loads:
                    raise ValueError(f"Anchor evidence does not match {mode} mode with {anchor_evidence_policy} policy in {log}")
                costs = [value for key, value in sample["metrics"].items()
                         if key.startswith("commit-read-cost.Construction.") and key.endswith(".elapsed_us")]
                if costs:
                    metrics = sample["metrics"]
                    accounted = metrics.get("commit-cost-accounting.accounted_us")
                    wall = metrics.get("commit-cost-accounting.construction_us")
                    gap = metrics.get("commit-cost-accounting.timer_gap_us")
                    if accounted is None or wall is None or gap is None or sum(costs) != accounted or wall - accounted != gap:
                        raise ValueError(f"Inconsistent exclusive construction accounting in {log}")
                run_samples += 1
                samples.append(sample)
                sample = None
        if sample is not None:
            raise ValueError(f"Unfinished sample in {log}")
        if run_samples != expected_samples:
            raise ValueError(f"Expected {expected_samples} complete samples per run, got {run_samples} in {log}")
    if len(samples) != len(logs) * expected_samples:
        raise ValueError(f"Expected {expected_samples} complete samples per run, got {len(samples)}")
    summary = {}
    for condition in ["first", "repeated"]:
        selected = [sample for sample in samples if sample["condition"] == condition]
        summary[condition] = {}
        for metric in sorted({key for sample in selected for key in sample["metrics"]}):
            values = [sample["metrics"][metric] for sample in selected if metric in sample["metrics"]]
            summary[condition][metric] = {"n": len(values), "median": statistics.median(values), "min": min(values), "max": max(values), "stdev": statistics.stdev(values) if len(values) > 1 else None}
    return {"variant": variant, "assessment": assessment, "mode": mode,
            "instrumentation": instrumentation, "preparation_substrate": preparation_substrate, "host_snapshot_policy": host_snapshot_policy,
            "anchor_evidence_policy": anchor_evidence_policy, "samples": samples, "summary": summary}


def locked_version(path, package):
    """Record the resolved runtime versions, rather than the manifest's version ranges."""
    match = re.search(r'\[\[package\]\]\nname = "' + re.escape(package) + r'"\nversion = "([^"]+)"',
                      path.read_text())
    if match is None:
        raise ValueError(f"No locked version of {package} in {path}")
    return match[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--samples", type=int, default=6)
    parser.add_argument("--subjects", type=int, default=1)
    parser.add_argument("--mode", choices=["create", "visualizer-slot", "descriptor", "new-version"], default="create", help="create: VisualizerUsage instances; visualizer-slot: VisualizerSlot instances; descriptor: one new HolonType descriptor and its affected saved Schema; new-version: one property-only successor of a saved non-root version")
    parser.add_argument("--anchor-evidence-policy", choices=["per-new-version", "retired"], default="retired", help="Expected evidence path in the guest artifact; saved in capture metadata")
    parser.add_argument("--variant", choices=["prefetch", "on-demand"], default="prefetch")
    parser.add_argument("--readiness-cases", action="store_true")
    parser.add_argument("--assessment", choices=["standard", "instance-experiment", "validator-package", "combined", "warm-probe"], default="standard", help="Label the separately built guest artifact; this does not select or build it")
    parser.add_argument("--link-profile", action="store_true", help="Capture read-only persisted link-query timings after Commit samples; replaces the retired --index-mode experiment")
    parser.add_argument("--instrumentation", choices=["822-v2", "historical"], default="822-v2")
    parser.add_argument("--preparation-substrate", choices=["broad-readiness", "declared-demand"], default="broad-readiness")
    parser.add_argument("--semantic-revision", default="3d053ba8becbac13496d7531abea8ce9a5303c53", help="Semantic revision represented by this capture")
    parser.add_argument("--governor-presence", default="unavailable", help="Governor entry presence established by workload setup")
    parser.add_argument("--guest-artifact-revision", default="unavailable", help="Revision used to build the supplied guest artifact")
    parser.add_argument("--registry-population", type=int, help="Requested setup population; observed values come from logs")
    parser.add_argument("--guest-cache-state", default="unavailable", help="Describe ingress and setup warming; a fresh transaction does not establish cold cache")
    parser.add_argument("--session-product-reuse", default="unavailable", help="Describe pre-existing Session products separately from transaction cache state")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--summarize-only", action="store_true", help="Re-summarize logs using their recorded metadata; older captures without a policy use per-new-version")
    args = parser.parse_args()
    if args.runs < 1 or args.samples < 2 or args.subjects < 1:
        parser.error("--runs must be positive and --samples must be at least two")
    if args.mode in ["descriptor", "new-version"] and (args.subjects != 1 or args.readiness_cases):
        parser.error(f"--mode {args.mode} measures one subject per Commit without readiness cases")
    if args.link_profile and args.mode != "create":
        parser.error("--link-profile supports only --mode create")
    if args.summarize_only:
        metadata = json.loads((args.output / "metadata.json").read_text())
        result = summarize(sorted(args.output.glob("run-*.log")), metadata["samples_per_run"],
                           metadata["variant"], metadata["assessment"], metadata.get("mode", "create"),
                           metadata.get("anchor_evidence_policy", "per-new-version"),
                           metadata.get("instrumentation", "historical"), metadata.get("preparation_substrate", "broad-readiness"), metadata.get("host_snapshot_policy", "optional"))
    else:
        env = {**os.environ, "MAP_COMMIT_PERF_SAMPLES": str(args.samples), "MAP_COMMIT_PERF_SUBJECTS": str(args.subjects), "MAP_PROFILE": "1", "RUST_LOG": "warn,holons_test=info,holons_client=info,commit_performance_tests=info,holons_guest=info,wasm=info", "WASM_LOG": "info"}
        if "MAP_COMMIT_PERF_INDEX_MODE" in env or "MAP_COMMIT_PERF_INDEX_OUTPUT" in env:
            parser.error("MAP_COMMIT_PERF_INDEX_* is retired; unset it and use --link-profile")
        env.pop("MAP_COMMIT_PERF_LINK_OUTPUT", None)
        env.pop("MAP_COMMIT_READINESS_CASES_OUTPUT", None)
        env["MAP_COMMIT_PERF_MODE"] = args.mode
        args.output.mkdir(parents=True, exist_ok=False)
        paths = [Path("happ/workdir/map_holons.dna"), Path("happ/Cargo.lock"), Path("host/Cargo.lock"), Path("Cargo.lock"), Path("flake.lock")]
        paths += sorted(Path("generated/core-schema-bootstrap").rglob("*.json"))
        paths += [Path(path) for path in output("git", "ls-files", "--modified", "--others", "--exclude-standard").splitlines() if Path(path).is_file()]
        paths = sorted(set(paths))
        metadata = {
            "semantic_revision": args.semantic_revision,
            "governor_presence": args.governor_presence,
            "instrumentation": args.instrumentation,
            "preparation_substrate": args.preparation_substrate,
            "guest_artifact_revision": args.guest_artifact_revision,
            "requested_registry_population": args.registry_population,
            "guest_cache_state": args.guest_cache_state,
            "session_product_reuse": args.session_product_reuse,
            "host_snapshot_policy": "required",
            "host_snapshot_observation": "ClientHolonService::commit_internal via Sweettest mock conductor; construction expected, production envelope encoding unavailable",
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "revision": output("git", "rev-parse", "HEAD"),
            "branch": output("git", "branch", "--show-current"),
            "status": output("git", "status", "--short"),
            "platform": platform.platform(), "machine": platform.machine(), "cpu_count": os.cpu_count(),
            "devdocs_revision": output("git", "-C", "../map-dev-docs", "rev-parse", "HEAD"),
            "rustc": output("rustc", "--version"), "cargo": output("cargo", "--version"),
            "runs": args.runs, "samples_per_run": args.samples, "subjects_per_commit": args.subjects, "mode": args.mode, "variant": args.variant, "link_profile": args.link_profile, "assessment": args.assessment,
            "readiness_cases": args.readiness_cases,
            "anchor_evidence_policy": args.anchor_evidence_policy,
            "package_substrate": "ValidatorPackage" if args.assessment in ["validator-package", "combined", "warm-probe"] else "DescriptorPackage",
            "runtime_versions": {"holochain": locked_version(Path("Cargo.lock"), "holochain"),
                                 "hdk": locked_version(Path("happ/Cargo.lock"), "hdk"),
                                 "hdi": locked_version(Path("happ/Cargo.lock"), "hdi")},
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
            if args.link_profile:
                run_env["MAP_COMMIT_PERF_LINK_OUTPUT"] = str((args.output / f"run-{run + 1:02}-links.json").resolve())
            with path.open("w") as log:
                result = subprocess.run(["cargo", "test", "--manifest-path", "tests/sweetests/Cargo.toml", "--test", "commit_performance_tests", "measures_first_and_repeated_visualizer_usage_commits", "--", "--exact", "--ignored", "--nocapture", "--test-threads=1"], stdout=log, stderr=subprocess.STDOUT, env=run_env)
            if result.returncode:
                print(f"Run failed; inspect {path}", file=sys.stderr)
                return result.returncode
            logs.append(path)
        result = summarize(logs, args.samples, args.variant, args.assessment, args.mode,
                           args.anchor_evidence_policy, args.instrumentation, args.preparation_substrate, "required")
    (args.output / "measurements.json").write_text(json.dumps(result, indent=2) + "\n")
    for condition, metrics in result["summary"].items():
        value = metrics["commit-baseline.command_us"]
        print(f"{condition}: n={value['n']} median={value['median']/1000:.3f} ms range={value['min']/1000:.3f}–{value['max']/1000:.3f} ms")
    print(f"Full measurements: {args.output / 'measurements.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
