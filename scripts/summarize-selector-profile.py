#!/usr/bin/env python3
"""Summarize MAP_PROFILE pretty span logs without adding nested durations together.

Usage: python3 scripts/summarize-selector-profile.py native.log [--request-id 42]
Metadata fetch counts are host service calls; they are not SQLite query counts.
"""
import argparse
import json
import re
from pathlib import Path

UNITS = {"ns": 0.000001, "µs": 0.001, "us": 0.001, "ms": 1, "s": 1000}
ROOTS = {"visualizer.select", "visualizer.discover", "visualizer.choose"}
FETCHES = {"storage.fetch_holon", "fetch_relationship", "storage.lookup_key"}


def summarize(text, request_ids=None):
    text = re.sub(r"\x1b\[[0-9;]*m", "", text)
    requests = {}
    ipc = {}
    for block in re.split(r"(?=^[ \t]*\d{4}-\d\d-\d\dT)", text, flags=re.M):
        if "map_profile: close," not in block:
            continue
        spans = re.findall(r"^\s*in map_profile::([\w.]+)(?: with ([^\n]*))?", block, re.M)
        roots = [(name, fields) for name, fields in spans if name in ROOTS]
        request = re.search(r"in map_profile::ipc.dispatch with request_id: (\d+)", block)
        if not spans or not request:
            continue
        request_id = int(request[1])
        if request_ids and request_id not in request_ids:
            continue
        duration = sum(float(value) * UNITS[unit] for value, unit in re.findall(
            r"time\.(?:busy|idle): ([\d.]+)(ns|µs|us|ms|s)", block.splitlines()[0]))
        if spans[0][0].startswith("ipc."):
            ipc.setdefault(request_id, {})[spans[0][0]] = duration
        if not roots:
            continue
        root, root_fields = roots[-1]
        kind = re.search(r"kind: (\w+)", root_fields)
        row = requests.setdefault(request_id, {"requestId": request_id, "operation": root,
            "kind": kind[1] if kind else None, "totalMs": None, "phases": {},
            "metadataFetches": 0, "conductorCalls": 0, "fetchesByPhaseAndIdentity": {}})
        name, fields = spans[0]
        if name == root:
            row["totalMs"] = duration
        elif name.startswith("visualizer."):
            phase = row["phases"].setdefault(name, {"count": 0, "inclusiveMs": 0})
            phase["count"] += 1
            phase["inclusiveMs"] += duration
        if name in FETCHES:
            row["metadataFetches"] += 1
            phase = next((span for span, _ in spans if span.startswith("visualizer.") and span not in ROOTS), root)
            identity = f"{phase} | {name} | {fields}"
            row["fetchesByPhaseAndIdentity"][identity] = row["fetchesByPhaseAndIdentity"].get(identity, 0) + 1
        if name == "conductor.zome_call":
            row["conductorCalls"] += 1
    for request_id, row in requests.items():
        row["ipcPhasesMs"] = ipc.get(request_id, {})
    return {"timingNote": "Phase timings include nested spans; do not sum them. totalMs comes only from the root span. Fetch counts are host-service calls, not SQL queries.",
        "requests": sorted(requests.values(), key=lambda row: row["requestId"])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--request-id", type=int, action="append")
    args = parser.parse_args()
    report = summarize(args.log.read_text(), args.request_id)
    print(json.dumps(report, indent=2))
    if not report["requests"]:
        parser.exit(1, "No correlated selector spans found; capture a navigation with MAP_PROFILE=1.\n")


if __name__ == "__main__":
    main()
