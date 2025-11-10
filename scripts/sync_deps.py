#!/usr/bin/env python3
"""
Synchronize workspace = true dependencies with explicit versions or paths
from the root Cargo.toml [workspace.dependencies] table.

Usage:
  python3 scripts/sync_deps.py [--dry-run] [--filter happ] [--verbose]
"""

import argparse
import difflib
from pathlib import Path
import tomlkit

ROOT_CARGO = Path("Cargo.toml")
TARGET_DIRS = ["happ", "host", "tests"]  # skip shared_crates


def load_workspace_deps():
    """Read [workspace.dependencies] from the root Cargo.toml."""
    if not ROOT_CARGO.exists():
        raise FileNotFoundError("Root Cargo.toml not found.")
    doc = tomlkit.parse(ROOT_CARGO.read_text())
    return doc.get("workspace", {}).get("dependencies", {})


def safe_relpath(path: Path) -> str:
    """Return path relative to repo root if possible."""
    try:
        return str(path.resolve().relative_to(Path.cwd().resolve()))
    except Exception:
        return str(path)


def apply_dependency_updates(manifest_path: Path, workspace_deps, dry_run=False, verbose=False):
    text = manifest_path.read_text()
    doc = tomlkit.parse(text)
    changed = False

    for section in ["dependencies", "dev-dependencies", "build-dependencies"]:
        if section not in doc:
            continue
        for dep_name, dep_spec in list(doc[section].items()):
            if verbose:
                print(f"🔍 Checking {dep_name} in [{section}] of {safe_relpath(manifest_path)} (type={type(dep_spec)})")
            if isinstance(dep_spec, dict) and dep_spec.get("workspace") is True:
                if dep_name in workspace_deps:
                    src_spec = workspace_deps[dep_name]

                    if isinstance(src_spec, dict):
                        # full table — copy all key/values
                        new_spec = tomlkit.table()
                        for k, v in src_spec.items():
                            new_spec[k] = v
                    else:
                        # simple version string — convert to { version = "..." }
                        new_spec = tomlkit.table()
                        new_spec["version"] = str(src_spec)

                    doc[section][dep_name] = new_spec

    if changed:
        updated = tomlkit.dumps(doc)
        if dry_run:
            print(f"\n--- {safe_relpath(manifest_path)}")
            for line in difflib.unified_diff(
                    text.splitlines(), updated.splitlines(),
                    fromfile="original", tofile="updated", lineterm=""
            ):
                print(line)
        else:
            manifest_path.write_text(updated)
            print(f"✅ Updated: {safe_relpath(manifest_path)}")


def find_target_manifests(filter_str=None):
    """Find Cargo.toml files under target directories."""
    results = []
    for d in TARGET_DIRS:
        base = Path(d)
        if not base.exists():
            continue
        for p in base.rglob("Cargo.toml"):
            if not filter_str or filter_str in str(p):
                results.append(p)
    return results


def main():
    parser = argparse.ArgumentParser(description="Sync workspace = true deps to explicit versions/paths.")
    parser.add_argument("--dry-run", action="store_true", help="Show changes without writing.")
    parser.add_argument("--filter", help="Only process manifests whose paths contain this string.")
    parser.add_argument("--verbose", action="store_true", help="Print detailed actions.")
    args = parser.parse_args()

    workspace_deps = load_workspace_deps()
    manifests = find_target_manifests(args.filter)

    print(f"📦 Loaded {len(workspace_deps)} workspace dependencies from root.")
    print(f"🧭 Found {len(manifests)} target Cargo.toml files to scan.\n")

    for manifest in manifests:
        apply_dependency_updates(manifest, workspace_deps, dry_run=args.dry_run, verbose=args.verbose)

    print("\n✅ Done.")


if __name__ == "__main__":
    main()
