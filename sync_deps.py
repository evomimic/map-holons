#!/usr/bin/env python3

import argparse
import tomlkit
from pathlib import Path
import difflib

# Configuration
ROOT_CARGO_TOML = Path("Cargo.toml")
SECTION_NAMES = ["dependencies", "dev-dependencies", "build-dependencies"]


# Helper: Load top-level workspace dependencies
def read_workspace_dependencies():
    with ROOT_CARGO_TOML.open("r") as f:
        doc = tomlkit.parse(f.read())
    return doc.get("workspace", {}).get("dependencies", {})


# Helper: Compute relative path from target manifest to dep path
def compute_relative_path(target_manifest: Path, dep_path: Path):
    try:
        return str(dep_path.resolve().relative_to(target_manifest.parent.resolve()))
    except ValueError:
        return str(Path.relpath(dep_path.resolve(), start=target_manifest.parent.resolve()))


# Helper: Update manifest contents
def update_manifest(manifest_path: Path, source_deps, dry_run=False, verbose=False):
    original = manifest_path.read_text()
    doc = tomlkit.parse(original)
    changed = False

    for section in SECTION_NAMES:
        if section not in doc:
            continue

        for dep_name, spec in list(doc[section].items()):
            if isinstance(spec, dict) and spec.get("workspace") is True and dep_name in source_deps:
                source_spec = source_deps[dep_name]

                # Copy original spec
                new_spec = tomlkit.table()

                # Resolve versioned deps
                if "version" in source_spec:
                    for k, v in source_spec.items():
                        new_spec[k] = v
                elif "path" in source_spec:
                    dep_path = Path(source_spec["path"])
                    rel_path = compute_relative_path(manifest_path, dep_path)
                    new_spec["path"] = rel_path

                # Replace inline or dotted entry
                doc[section][dep_name] = new_spec
                changed = True

                if verbose:
                    print(f"🔄 Replaced [{section}].{dep_name} in {manifest_path}")

    updated = tomlkit.dumps(doc)

    if dry_run:
        if original != updated:
            print(f"\n🔍 Diff for {manifest_path}:")
            diff = difflib.unified_diff(
                original.splitlines(),
                updated.splitlines(),
                fromfile=f"{manifest_path} (original)",
                tofile=f"{manifest_path} (updated)",
                lineterm=""
            )
            print("\n".join(diff))
        else:
            print(f"✅ No changes needed: {manifest_path}")
    else:
        if changed:
            manifest_path.write_text(updated)
            print(f"✅ Updated: {manifest_path}")


# Discover all Cargo.toml files except top-level
def find_all_manifests():
    return [
        p for p in Path(".").rglob("Cargo.toml")
        if p.resolve() != ROOT_CARGO_TOML.resolve()
    ]


# CLI Args
def parse_args():
    parser = argparse.ArgumentParser(description="Replace workspace=true deps with concrete paths/versions")
    parser.add_argument("--dry-run", action="store_true", help="Show diff without writing changes")
    parser.add_argument("--filter", metavar="NAME", help="Only update files containing NAME in the path")
    parser.add_argument("--verbose", action="store_true", help="Print detailed actions")
    return parser.parse_args()


def main():
    args = parse_args()

    if not ROOT_CARGO_TOML.exists():
        print("❌ Top-level Cargo.toml not found.")
        return

    source_deps = read_workspace_dependencies()
    targets = find_all_manifests()

    for manifest in targets:
        if args.filter and args.filter not in str(manifest):
            continue
        update_manifest(manifest, source_deps, dry_run=args.dry_run, verbose=args.verbose)


if __name__ == "__main__":
    main()
