#!/usr/bin/env python3
"""
sync_deps_explicit.py

Expands `{ workspace = true }` dependencies in happ/ and tests/ crates
to explicit `version = "..."` or `path = "../..."` specs based on the root workspace.

Usage:
  python3 scripts/sync_deps_explicit.py --dry-run
  python3 scripts/sync_deps_explicit.py --verbose
  python3 scripts/sync_deps_explicit.py --write
"""

import argparse
import difflib
from pathlib import Path
import tomlkit
import os

ROOT = Path(__file__).resolve().parents[1]
FIELDS_TO_SYNC = ["dependencies", "dev-dependencies", "build-dependencies"]

TARGET_DIRS = [ROOT / "happ", ROOT / "tests"]
ROOT_CARGO = ROOT / "Cargo.toml"


# ──────────────────────────────────────────────────────────────
# Helpers
# ──────────────────────────────────────────────────────────────

def load_workspace_dependencies():
    """Load all `[workspace.dependencies]` from the root Cargo.toml."""
    if not ROOT_CARGO.exists():
        raise FileNotFoundError(f"❌ Root Cargo.toml not found at {ROOT_CARGO}")
    doc = tomlkit.parse(ROOT_CARGO.read_text())
    return doc.get("workspace", {}).get("dependencies", {})


def compute_relative_path(from_manifest: Path, dep_path: str) -> str:
    """Compute a relative path between manifest and dep, even across directories."""
    from_dir = from_manifest.parent.resolve()
    dep_abs = (ROOT / dep_path).resolve()
    try:
        # Works even if dep is outside from_dir
        rel = os.path.relpath(dep_abs, start=from_dir)
        return rel
    except Exception as e:
        print(f"  ⚠️ Failed to compute relative path from {from_dir} to {dep_abs}: {e}")
        return dep_path


def expand_workspace_deps(doc, section, workspace_deps, manifest_path, verbose=False):
    """Replace `{ workspace = true }` with explicit version/path from root workspace."""
    if section not in doc:
        return False

    deps = doc[section]
    changed = False

    for dep_name, spec in list(deps.items()):
        if not (isinstance(spec, dict) and spec.get("workspace") is True):
            continue

        if dep_name not in workspace_deps:
            if verbose:
                print(f"  ⚠️ Skipping {dep_name}: not found in workspace dependencies.")
            continue

        source = workspace_deps[dep_name]
        new_spec = tomlkit.inline_table()

        if "version" in source:
            new_spec["version"] = source["version"]
        elif "path" in source:
            rel_path = compute_relative_path(manifest_path, source["path"])
            new_spec["path"] = rel_path
        else:
            if verbose:
                print(f"  ⚠️ Skipping {dep_name}: workspace spec has no path or version.")
            continue

        # Copy over any feature flags if present in workspace spec
        if "features" in source:
            new_spec["features"] = source["features"]

        deps[dep_name] = new_spec
        changed = True

        if verbose:
            print(f"  🔄 Expanded {dep_name}: {{ workspace = true }} → {dict(new_spec)}")

    return changed


def process_manifest(manifest_path: Path, workspace_deps, dry_run=False, verbose=False):
    """Process one Cargo.toml file and expand workspace deps."""
    text = manifest_path.read_text()
    doc = tomlkit.parse(text)
    changed = False

    if verbose:
        print(f"\n📄 Processing: {manifest_path.relative_to(ROOT)}")

    for section in FIELDS_TO_SYNC:
        if expand_workspace_deps(doc, section, workspace_deps, manifest_path, verbose):
            changed = True

    if not changed:
        if verbose:
            print("  ✅ No workspace deps to expand.")
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


# ──────────────────────────────────────────────────────────────
# Main
# ──────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Expand workspace=true deps to explicit specs in happ/ and tests/")
    parser.add_argument("--dry-run", action="store_true", help="Show diffs without writing changes")
    parser.add_argument("--verbose", action="store_true", help="Print detailed logs")
    parser.add_argument("--write", action="store_true", help="Apply changes instead of dry run")
    args = parser.parse_args()

    workspace_deps = load_workspace_dependencies()
    manifests = []

    for tdir in TARGET_DIRS:
        if tdir.exists():
            manifests += list(tdir.rglob("Cargo.toml"))

    print(f"🧭 Found {len(manifests)} manifests in happ/ and tests/")

    any_changed = False
    for manifest in manifests:
        if process_manifest(manifest, workspace_deps, dry_run=not args.write, verbose=args.verbose):
            any_changed = True

    if not any_changed:
        print("✅ No updates needed.")
    else:
        print("✨ Done.")


if __name__ == "__main__":
    main()
