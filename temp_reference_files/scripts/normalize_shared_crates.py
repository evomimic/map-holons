#!/usr/bin/env python3
"""
normalize_shared_crates.py
--------------------------------
Normalizes Cargo.toml files inside `shared_crates/` so that any dependencies
that also exist in the root `[workspace.dependencies]` are replaced with:

    dep_name = { workspace = true }

Supports:
  ✅ inline tables  (e.g. { version = "1", features = ["derive"] })
  ✅ path tables    (e.g. { path = "../core" })
  ✅ string specs   (e.g. dep = "0.1")

Safe: skips dependencies not defined in the root workspace.
"""

import tomlkit
from pathlib import Path
import difflib
import sys

ROOT = Path("Cargo.toml")
SHARED = Path("shared_crates")
FIELDS = ["dependencies", "dev-dependencies", "build-dependencies"]


def load_workspace_deps():
    """Load `[workspace.dependencies]` from the root Cargo.toml."""
    doc = tomlkit.parse(ROOT.read_text())
    ws_deps = doc.get("workspace", {}).get("dependencies", {})
    return set(ws_deps.keys())


def normalize_dependencies(doc, section, workspace_deps, verbose=False):
    """Normalize inline/path/string dependencies that exist in workspace deps."""
    if section not in doc:
        return False

    deps = doc[section]
    changed = False

    for name, val in list(deps.items()):
        # Skip if not part of the workspace
        if name not in workspace_deps:
            continue

        # ✅ Already normalized (nothing to do)
        if isinstance(val, tomlkit.items.InlineTable) and val.get("workspace") is True:
            continue

        # Inline table (e.g. { version = "...", ... })
        if isinstance(val, tomlkit.items.InlineTable):
            if verbose:
                print(f"  🔄 Normalizing {name}: {dict(val)} → {{ workspace = true }}")
            deps[name] = tomlkit.inline_table()
            deps[name]["workspace"] = True
            changed = True

        # Plain table ([dependencies.foo])
        elif isinstance(val, tomlkit.items.Table):
            if verbose:
                print(f"  🔄 Normalizing {name}: [table] → {{ workspace = true }}")
            deps[name] = tomlkit.inline_table()
            deps[name]["workspace"] = True
            changed = True

        # String value ("0.1")
        elif isinstance(val, tomlkit.items.String):
            if verbose:
                print(f"  🔄 Normalizing {name}: \"{val}\" → {{ workspace = true }}")
            deps[name] = tomlkit.inline_table()
            deps[name]["workspace"] = True
            changed = True

    return changed


def process_manifest(manifest_path, workspace_deps, dry_run=False, verbose=False):
    """Process a single Cargo.toml and optionally write changes."""
    repo_root = Path(__file__).resolve().parent.parent  # resolve repo root dynamically

    try:
        rel_path = manifest_path.resolve().relative_to(repo_root)
    except ValueError:
        rel_path = manifest_path.name  # fallback if outside repo

    if verbose:
        print(f"📄 Processing: {rel_path}")

    text = manifest_path.read_text()
    doc = tomlkit.parse(text)
    changed = False

    for section in FIELDS:
        if normalize_dependencies(doc, section, workspace_deps, verbose):
            changed = True

    if not changed:
        return False

    updated = tomlkit.dumps(doc)
    if dry_run:
        diff = "".join(
            difflib.unified_diff(
                text.splitlines(),
                updated.splitlines(),
                fromfile=f"{rel_path} (original)",
                tofile=f"{rel_path} (updated)",
                lineterm="",
            )
        )
        if diff.strip():
            print(f"\n📄 Diff for {rel_path}:")
            print(diff)
        else:
            manifest_path.write_text(updated)
            print(f"✅ Updated: {rel_path}")

        return True


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Normalize shared_crates Cargo.toml to use workspace = true")
    parser.add_argument("--dry-run", action="store_true", help="Show changes without writing them")
    parser.add_argument("--verbose", action="store_true", help="Print detailed actions")
    parser.add_argument("--write", action="store_true", help="Actually modify files")
    args = parser.parse_args()

    workspace_deps = load_workspace_deps()
    manifests = sorted(SHARED.rglob("Cargo.toml"))
    print(f"🧭 Found {len(manifests)} manifests under shared_crates/")

    any_changes = False
    for manifest in manifests:
        changed = process_manifest(manifest, workspace_deps, dry_run=args.dry_run, verbose=args.verbose)
        any_changes = any_changes or changed
        sys.stdout.flush()  # ensure no buffered newlines accumulate

    if any_changes:
        if args.dry_run:
            print(f"⚠️ Changes *would* be made in one or more of {len(manifests)} manifests.")
        else:
            print(f"⚠️ Changes applied in one or more of {len(manifests)} manifests.")
    elif args.verbose:
        print(f"✅ No changes made across {len(manifests)} manifests.")

    return any_changes


if __name__ == "__main__":
    script_name = Path(__file__).stem
    changed = main()
    status = "⚠️ Changes detected (see above)" if changed else "✅ No updates needed"
    print(f"🔹 {script_name}: {status}")
