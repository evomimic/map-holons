# 🤝 Contributing to MAP Holons

Thank you for your interest in contributing to the MAP Holons project — the core foundation of the Memetic Activation
Platform (MAP).

This guide will help you set up your development environment, understand the project layout, and begin contributing
effectively.

---

## 🧰 Development Setup

We use a multi-workspace Rust project structure with [Holonix](https://github.com/holochain/holonix) for environment
consistency.

### 1. Install the Holochain dev environment

Follow the instructions here:  
👉 https://developer.holochain.org/docs/install/

### 2. Enter the development shell

From the root of the repository:

```bash
nix develop
npm install
```

> 💡 All other commands should be run **from within the nix shell**.

### 3. Configure your IDE

Make sure your IDE is configured to support multiple independent Cargo workspaces:

👉 [IDE Setup Guide](IDE_SETUP.md)

---

## 📦 Project Layout

Start with [ARCHITECTURE.md](ARCHITECTURE.md) for the repo’s execution-context model and workspace boundaries.

| Directory         | Purpose                                                       |
|-------------------|---------------------------------------------------------------|
| `happ/`           | Holochain app Rust workspace for WASM builds                  |
| `host/`           | Native host workspace for Rust, orchestration, and UI         |
| `shared_crates/`  | Shared Rust libraries compiled into `happ` and `host`         |
| `tests/`          | Test harnesses and standalone test crates such as sweetests   |
| root workspace    | npm orchestration, IDE support, and dependency coordination   |

---

## 🧪 Running Tests

To run backend tests (e.g. sweetests):

```bash
npm run test
```

Or run just the integration tests:

```bash
npm run sweetest
```

---

## 🚀 Dev Mode (Fast Restart Loop)

When iterating on Holochain zome logic locally, you can skip lair keystore startup and reset conductor state automatically on each run by setting:

```sh
HC_DEV_MODE=1
```

This gives significantly faster restart loops by using an ephemeral in-memory keystore and preserving the compiled WASM cache across restarts.

See [DEV_MODE.md](host/conductora/plugins/tauri-plugin-holochain/DEV_MODE.md) for full details including timing expectations, network config, and useful log lines.

---

## ✅ Contribution Guidelines

- Keep shared crates (`holons-core`, etc.) free of Tokio or native-only dependencies.
- Run `cargo check --target wasm32-unknown-unknown` for any crate used in `wasm/`.
- Write clear, well-scoped commits with informative messages.
- Prefer small, targeted PRs over large, monolithic changes.
- Include tests when possible — especially for new functionality.

---

---

## 🧪 Continuous Integration (CI) Checks

All pull requests are automatically validated by our GitHub Actions CI workflows.

### ✅ What the CI Checks Do

- **Light** — Runs the SDK wire-fixture and schema checks, then `npm run test:light` (all unit tests: shared crates, host, hApp, sweetest harness lib, web, SDK). No Holochain conductor.
- **Heavy** — Runs `npm run test:heavy`: builds the hApp and probes, then runs the sweetests under `cargo-nextest`, which prints each test's duration and terminates tests that hang. Heavy runs as parallel shards; which test files each shard runs is defined by the `ci-<shard>` profiles in `.config/nextest.toml`. Then re-audits the packaged coordinator surfaces.
- **Format** — Runs `npm run fmt:check` inside `nix develop` to enforce Rust formatting across `host`, `happ`, and `tests/sweetests`
- **CI Pass Aggregator** — Combines and reports status of all required checks

Light and Heavy run in parallel, so unit test failures are reported without waiting for the sweetests. `npm test` still runs both tiers locally.

---

### 💡 Before You Push

To avoid failed checks:

- Run `npm run fmt` locally to format all Rust code covered by the repo-level formatting contract (`host`, `happ`, and `tests/sweetests`)
- Run `npm run fmt:check` if you want the same formatting validation used in CI
- Ensure unit tests pass with `npm run test:light` and integration tests with `npm run sweetest`
- Keep commits clean and scoped — large formatting-only changes should be separated

---


## 📚 Reference Docs

- 🧠 [MAP Core Developer Docs](https://memetic-activation-platform.github.io/map-dev-docs/core/)
- 🧬 [mApp Developer Docs](https://memetic-activation-platform.github.io/map-dev-docs/mapp/)
- 🛠 [IDE Setup Guide](IDE_SETUP.md)

---

## 🙏 Thank You!

Your contributions help build a more composable, autonomous, and expressive world of software. 💫
