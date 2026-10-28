#!/usr/bin/env python3
"""Capture isolated EffectiveHolonDescriptor construction and validation costs inside Nix.

Example: nix develop --command python3 scripts/measure-effective-descriptor-construction.py
         --output /tmp/effective-descriptor-costs --samples 3
Use --without-contributions for a benchmark-only graph without provenance holons.
Normal guest artifacts are restored after the capture, including on failure.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess


def analyze(output):
    data = json.loads((output / "capture.json").read_text())
    details = []
    pending_details = []
    phases = {name: [] for name in ("core", "envelope")}
    for line in (output / "capture.log").read_text().splitlines():
        if "[effective-descriptor-detail]" in line:
            fields = dict(re.findall(r"(\w+)=([^\s]*)", line.split("[effective-descriptor-detail]", 1)[1]))
            pending_details.append({key: (value if key in ("phase", "collection") else int(value)) for key, value in fields.items()})
            continue
        if "[effective-descriptor-profile]" not in line:
            continue
        fields = dict(re.findall(r"(\w+)=([\w.-]+)", line.split("[effective-descriptor-profile]", 1)[1]))
        phase = fields.pop("phase")
        if fields.pop("success") != "true":
            raise ValueError(f"Failed construction: {line}")
        if phase == "core":
            details.append(pending_details)
            pending_details = []
        phases[phase].append({key: int(value) for key, value in fields.items()})
    rows = data["rows"]
    expected = data["samples_per_target_and_mode"] * 3 * 2
    if len(rows) != expected or any(len(values) != expected for values in phases.values()):
        raise ValueError("Incomplete host or guest capture; rebuild with descriptor-performance-profiling")
    for row, core, envelope, detail in zip(rows, phases["core"], phases["envelope"], details):
        row["construction_detail"] = detail
        row.update(guest_core=core, guest_envelope=envelope)
        row["outside_guest_envelope_us"] = row["host_construction_us"] - envelope["total_us"]
        if core["allocations"] != row["graph_nodes"] or envelope["execute_us"] < core["elapsed_us"] or row["outside_guest_envelope_us"] < 0:
            raise ValueError("Inconsistent phase or allocation evidence")
    summary = []
    for target in dict.fromkeys(row["target"] for row in rows):
        case = [row for row in rows if row["target"] == target]
        if any(row["product_counts"] != case[0]["product_counts"] or row["contributions"] != case[0]["contributions"] for row in case):
            raise ValueError(f"Result shape changed between samples: {target}")
        for mode in ("fresh_transaction", "retained_transaction"):
            group = [row for row in case if row["mode"] == mode]
            item = dict(target=target, mode=mode, samples=len(group), graph_nodes=group[0]["graph_nodes"],
                        contributions=group[0]["contributions"], product_counts=group[0]["product_counts"])
            metrics = {
                "host": lambda row: row["host_construction_us"],
                "guest_core": lambda row: row["guest_core"]["elapsed_us"],
                "guest_input_validation": lambda row: row["guest_envelope"]["request_validation_us"],
                "guest_response_projection": lambda row: row["guest_envelope"]["response_projection_us"],
                "guest_total": lambda row: row["guest_envelope"]["total_us"],
                "outside_guest": lambda row: row["outside_guest_envelope_us"],
            }
            for name, extract in metrics.items():
                values = [extract(row) for row in group]
                item[name + "_ms"] = dict(median=statistics.median(values) / 1000,
                                          min=min(values) / 1000, max=max(values) / 1000)
            item["core_link_queries"] = [row["guest_core"]["link_queries"] for row in group]
            item["transients_before"] = [row["transients_before"] for row in group]
            item["session_json_bytes_before"] = [row["session_json_bytes_before"] for row in group]
            item["session_json_bytes_after"] = [row["session_json_bytes_after"] for row in group]
            item["construction_phases"] = {}
            for phase in dict.fromkeys(detail["phase"] for row in group for detail in row["construction_detail"]):
                measurements = [{metric: sum(d[metric] for d in row["construction_detail"] if d["phase"] == phase)
                                 for metric in ("elapsed_us", "link_queries", "returned_links")} for row in group]
                item["construction_phases"][phase] = {metric: statistics.median(m[metric] for m in measurements)
                                                       for metric in ("elapsed_us", "link_queries", "returned_links")}
            summary.append(item)
    report = dict(contribution_holons=data.get("include_contributions", False), setup_ms=data["setup_us"] / 1000, notes=data["notes"], summary=summary, rows=rows)
    (output / "analysis.json").write_text(json.dumps(report, indent=2) + "\n")
    for item in summary:
        print(f"{item['target']} {item['mode']}: host={item['host_ms']['median'] / 1000:.3f}s "
              f"core={item['guest_core_ms']['median'] / 1000:.3f}s "
              f"projection={item['guest_response_projection_ms']['median'] / 1000:.3f}s "
              f"nodes={item['graph_nodes']}")
    print(f"Results: {output / 'analysis.json'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--analyze-only", action="store_true", help="summarize an existing capture without running a conductor")
    args = parser.parse_args()
    output = args.output.resolve()
    if args.analyze_only:
        analyze(output)
        return
    if not 2 <= args.samples <= 10:
        parser.error("samples must be 2..10")
    if not os.environ.get("IN_NIX_SHELL"):
        parser.error("run this capture inside nix develop")
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        parser.error("use an empty output directory to preserve prior evidence")
    root = Path(__file__).resolve().parent.parent
    metadata = dict(platform=platform.platform(), samples=args.samples,
                    revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
                    rust=subprocess.check_output(["rustc", "--version"], text=True).strip())
    lock = (root / "Cargo.lock").read_text()
    metadata["holochain_version"] = re.search(r'\[\[package\]\]\nname = "holochain"\nversion = "([^"]+)"', lock).group(1)
    sources = [Path(__file__).resolve(), root / "tests/sweetests/tests/effective_descriptor_performance_tests.rs",
               root / "shared_crates/holons_core/src/descriptors/effective_descriptor.rs",
               root / "happ/crates/holons_guest/src/dances_guest/dance_adapter.rs",
               root / "happ/crates/holons_guest/src/guest_shared_objects/guest_holon_service.rs"]
    metadata["source_sha256"] = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sources}
    (output / "working-tree.patch").write_bytes(subprocess.check_output(["git", "diff", "HEAD"], cwd=root))
    environment = dict(os.environ, MAP_EFFECTIVE_DESCRIPTOR_PERF_OUTPUT=str(output / "capture.json"),
                       MAP_EFFECTIVE_DESCRIPTOR_PERF_SAMPLES=str(args.samples), RUST_LOG="warn", WASM_LOG="warn")
    feature = "holons/descriptor-performance-profiling"
    try:
        with (output / "profile-build.log").open("w") as log:
            subprocess.run(["cargo", "build", "-p", "holons_integrity", "-p", "holons", "--release",
                            "--target", "wasm32-unknown-unknown", "--features", feature],
                           cwd=root / "happ", env=dict(os.environ, CARGO_TARGET_DIR="target"),
                           stdout=log, stderr=subprocess.STDOUT, check=True)
            subprocess.run(["hc", "app", "pack", "workdir", "--recursive"], cwd=root / "happ",
                           stdout=log, stderr=subprocess.STDOUT, check=True)
        metadata["profile_dna_sha256"] = hashlib.sha256((root / "happ/workdir/map_holons.dna").read_bytes()).hexdigest()
        (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
        with (output / "capture.log").open("w") as log:
            subprocess.run(["cargo", "test", "--manifest-path", "tests/sweetests/Cargo.toml", "--test",
                            "effective_descriptor_performance_tests", "--", "--ignored", "--nocapture"],
                           cwd=root, env=environment, stdout=log, stderr=subprocess.STDOUT, check=True)
        analyze(output)
    finally:
        with (output / "restore-build.log").open("w") as log:
            subprocess.run(["npm", "run", "build:happ"], cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True)


if __name__ == "__main__":
    main()
