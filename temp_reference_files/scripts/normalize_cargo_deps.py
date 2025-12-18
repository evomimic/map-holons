#!/usr/bin/env python3
"""
normalize_cargo_deps.py

Normalizes section-table dependency declarations in Cargo.toml files
(e.g. [dependencies.foo]) into inline { path = "..."} form.

Usage:
  python3 scripts/normalize_cargo_deps.py --dry-run
  python3 scripts/normalize_cargo_deps.py --verbose
  python3 scripts/normalize_cargo_deps.py --filter holons_core
"""

import argparse
import difflib
from pathlib import Path
import tomlkit

# Constants
ROOT = Path(__file__).resolve().parents[1]
FIELDS_TO_CONVERT = ["dependencies", "dev-dependencies", "build-dependencies"]


def normalize_dependencies(doc, section, verbose=False):
    """Convert [dependencies.foo] tables into inline { ... } syntax."""
    if section not in doc:
        if verbose:
            print(f"  (no explicit [{section}] section)")
        return False

    changed = False
    deps = doc[section]
    found = []

    if verbose:
        print(f"  🔍 Scanning for section-table deps in [{section}] ...")

    for name, spec in list(deps.items()):
        if isinstance(spec, tomlkit.items.Table) and not isinstance(spec, tomlkit.items.InlineTable):
            inline = tomlkit.inline_table()
            for k, v in spec.items():
                inline[k] = v
            deps[name] = inline
            found.append((name, dict(spec)))
            changed = True

    if verbose:
        if found:
            print(f"  🧩 Found section-table deps in [{section}]:")
            for n, s in found:
                if "path" in s:
                    print(f"     → {n}: path = {s['path']}")
                else:
                    print(f"     → {n}: {s}")
        else:
            print(f"  ✅ No section-table dependencies found in [{section}].")
        print(f"  ✅ Explicit [{section}] keys: {list(deps.keys())}")

    return changed


def process_manifest(manifest_path: Path, dry_run=False, verbose=False):
    """Process a single Cargo.toml file."""
    if verbose:
        rel_path = manifest_path.relative_to(ROOT)
        print(f"\n📄 Processing manifest: {rel_path}")

    text = manifest_path.read_text()
    doc = tomlkit.parse(text)
    changed = False

    for section in FIELDS_TO_CONVERT:
        if normalize_dependencies(doc, section, verbose=verbose):
            changed = True

    if changed:
        updated = tomlkit.dumps(doc)
        if dry_run:
            diff = difflib.unified_diff(
                text.splitlines(),
                updated.splitlines(),
                fromfile=f"{manifest_path} (original)",
                tofile=f"{manifest_path} (updated)",
                lineterm=""
            )
            diff_output = "\n".join(diff)
            if diff_output.strip():
                print(diff_output)
        else:
            manifest_path.write_text(updated)
            print(f"✅ Updated: {manifest_path.relative_to(ROOT)}")

    elif verbose:
        print(f"  ✅ No changes needed.")

    return changed


def find_target_manifests(filter_str=None):
    """Locate Cargo.toml files recursively (excluding root)."""
    manifests = [
        p for p in Path(ROOT).rglob("Cargo.toml")
        if p.name == "Cargo.toml" and p != ROOT / "Cargo.toml"
    ]
    if filter_str:
        manifests = [m for m in manifests if filter_str in str(m)]
    return manifests


def main():
    parser = argparse.ArgumentParser(description="Normalize section-table dependencies into inline form")
    parser.add_argument("--dry-run", action="store_true", help="Show diff without writing changes")
    parser.add_argument("--filter", metavar="NAME", help="Only update manifests whose path contains NAME")
    parser.add_argument("--verbose", action="store_true", help="Print detailed actions")
    args = parser.parse_args()

    targets = find_target_manifests(args.filter)
    print(f"🧭 Found {len(targets)} Cargo.toml manifests.")
    if not targets:
        print("⚠️ No Cargo.toml files found matching filter.")
        return False

    any_changes = False
    for manifest in targets:
        if process_manifest(manifest, dry_run=args.dry_run, verbose=args.verbose):
            any_changes = True

    return any_changes


if __name__ == "__main__":
    script_name = Path(__file__).stem
    changed = main()
    status = "⚠️ Changes detected (see above)" if changed else "✅ No updates needed"
    print(f"🔹 {script_name}: {status}")
