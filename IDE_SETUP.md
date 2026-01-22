# IDE Setup for MAP Holons Development

This repository uses **multiple independent Cargo workspaces** (`native/`, `wasm/`, `test/`) to isolate code for native
runtimes, WASM runtimes, and test execution.

To get full IntelliSense, diagnostics, and navigation across crates, you’ll need to configure your IDE to recognize all
workspace roots.

This guide includes setup instructions for both:

- [VSCode + Rust Analyzer](#vscode--rust-analyzer-setup)
- [RustRover (JetBrains)](#rustrover--jetbrains-setup)

---

## VSCode + Rust Analyzer Setup

> ⚠️ Rust Analyzer **does not auto-discover multiple independent Cargo workspaces**. You must explicitly link them.

### 1. Open the repo root in VSCode

Even though there is no top-level `Cargo.toml`, this is fine — you'll point Rust Analyzer to the actual workspace
manifests.

### 2. Add a `.vscode/settings.json` (or update existing)

```json
{
  "rust-analyzer.linkedProjects": [
    "native/Cargo.toml",
    "wasm/Cargo.toml",
    "test/Cargo.toml"
  ],
  "rust-analyzer.cargo.buildScripts.enable": true,
  "rust-analyzer.checkOnSave.command": "clippy"
}
```

> If `.vscode/` doesn’t exist, create it in the repo root.

### 3. Restart VSCode or the Rust Analyzer server

You should now see:

- Cross-workspace type resolution
- Auto-imports across crates
- Accurate diagnostics for `Cargo.toml` issues
- Go-to-definition across the workspace boundary

---

## Notes for VSCode Users

- Crates that are **not listed in the linked projects** won’t be indexed.
- You must use `path = "../../crates/..."` correctly for inter-crate dependencies.
- `cargo test`, `cargo build`, etc. should be run using the correct `--manifest-path` if needed.

---

## RustRover / IntelliJ (JetBrains) Setup

RustRover supports **multiple workspaces** natively, but you may need to **attach them manually**.

### 1. Open the repo root as your JetBrains project

You can open `map-holons/` directly, even though it doesn’t have a top-level `Cargo.toml`.

### 2. Go to `Preferences` (or `Settings`) → `Languages & Frameworks` → `Rust`

Under **Cargo Projects**, ensure you attach the following:

- `native/Cargo.toml`
- `wasm/Cargo.toml`
- `test/Cargo.toml`

If any are missing:

- Click **➕ (Add)** and navigate to the appropriate manifest.

### 3. Let RustRover index the project

Once complete, you will have:

- Cross-workspace IntelliSense
- Navigation and symbol resolution between crates
- Support for run/debug configurations per workspace

---

## Optional: Add Custom Run Configurations

You may want to create named run/test configs:

- 🧪 `Test (native)` → Run tests from `test/`
- 🧱 `Build (wasm)` → Build zome crates for WASM
- 🖥 `Run (client)` → Run native `holons_client` targets

> You can add these under `Run → Edit Configurations`, using the correct working directory and command for each
> workspace.

---

## Best Practices (Applies to All IDEs)

- Use relative `path = ...` in `Cargo.toml` to link internal crates
- Avoid running `cargo build --workspace` from repo root — use per-workspace `Cargo.toml`
- Add `.dev/` to `.gitignore` if you create scratchpad files or developer-specific overrides

---

## Related

- [Workspace Strategy Overview](./WORKSPACE_LAYOUT.md)
- [Environment Setup](./ENVIRONMENT.md)
- [Contributing Guide](CONTRIBUTING.md)