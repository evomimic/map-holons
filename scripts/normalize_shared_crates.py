#!/usr/bin/env python3
"""
normalize_shared_crates.py

Convert explicit dependency specs in shared crates (e.g. `{ path = "../..." }`
or `{ version = "1" }`) into workspace references (`{ workspace = true }`).

Usage:
  python3 scripts/normalize_shared_crates.py --dry-run
  python3 scripts/normalize_shared_crates.py --verbose
  python3 scripts/normalize_shared_crates.py --write
"""

import argparse
import difflib
from pathlib import Path
import tomlkit

ROOT = Path(__file__).resolve().parents[1]
SHARED_ROOT = ROOT / "shared_crates"
FIELDS_TO_CONVERT = ["dependencies", "dev-dependencies", "build-dependencies"]


def make_workspace_inline():
    """Return a `{ workspace = true }` inline table."""
    t = tomlkit.inline_table()
    t["workspace"] = True
    return t


def normalize_dependencies(doc, section, verbose=False):
    """Replace inline or path-table dependencies with `{ workspace = true }`."""
    if section not in doc:
        return False

    changed = False
    deps = doc[section]

    for name, spec in list(deps.items()):
        # Skip if already workspace=true
        if isinstance(spec, dict) and spec.get("workspace") is True:
            continue

        # Convert if explicit path or version found
        if isinstance(spec, dict) and ("path" in spec or "version" in spec):
            if verbose:
                print(f"  🔄 Normalizing {name}: {spec} → {{ workspace = true }}")
            deps[name] = make_workspace_inline()
            changed = True

    return changed


def process_manifest(manifest_path, dry_run=False, verbose=False):
    """Normalize a single Cargo.toml file."""
    text = manifest_path.read_text()
    doc = tomlkit.parse(text)
    changed = False

    if verbose:
        print(f"\n📄 Processing: {manifest_path.relative_to(ROOT)}")

    for section in FIELDS_TO_CONVERT:
        if normalize_dependencies(doc, section, verbose):
            changed = True

    if not changed:
        if verbose:
            print("  ✅ No changes needed.")
        return False

    updated = tomlkit.dumps(doc)
    if dry_run:
        diff = difflib.unified_diff(
            text.splitlines(),
            updated.splitlines(),
            fromfile=f"{manifest_path} (original)",
            tofile=f"{manifest_path} (updated)",
            lineterm=""
        )
        print("\n".join(diff))
    else:
        manifest_path.write_text(updated)
        print(f"✅ Updated: {manifest_path.relative_to(ROOT)}")

    return True


def main():
    parser = argparse.ArgumentParser(description="Normalize shared crate dependencies to `{ workspace = true }`")
    parser.add_argument("--dry-run", action="store_true", help="Show diffs without writing changes")
    parser.add_argument("--verbose", action="store_true", help="Print detailed logs")
    parser.add_argument("--write", action="store_true", help="Apply changes instead of dry run")
    args = parser.parse_args()

    if not SHARED_ROOT.exists():
        print(f"❌ shared_crates/ not found at {SHARED_ROOT}")
        return

    manifests = list(SHARED_ROOT.rglob("Cargo.toml"))
    print(f"🧭 Found {len(manifests)} manifests under shared_crates/")

    any_changed = False
    for manifest in manifests:
        if process_manifest(manifest, dry_run=not args.write, verbose=args.verbose):
            any_changed = True

    if not any_changed:
        print("✅ No updates needed.")
    else:
        print("✨ Done.")


if __name__ == "__main__":
    main()
