# Map Holons

[![Docs](https://img.shields.io/badge/Developer%20Docs-📘%20Open-blue)](https://memetic-activation-platform.github.io/map-dev-docs/)

MAP Holons provides the foundational layer of the _Memetic Activation Platform_ (MAP). This layer provides storage,
retrieval and querying of self-describing, active holons.

MAP Holons is still in early proof-of-concept prototype stages.

📚 **Developer Documentation**

- 👉 [MAP Core Developer Docs](https://memetic-activation-platform.github.io/map-dev-docs/core/) — for contributors to
  the MAP Holons codebase (Rust, Holochain, architecture)
- 👉 [mApp Developer Docs](https://memetic-activation-platform.github.io/map-dev-docs/mapp/) — for developers building
  applications on top of MAP Holons
- 🛠️ [IDE Setup Guide](IDE_SETUP.md) — how to configure RustRover or VSCode for multi-workspace support
- 🙌 [Contributing Guide](CONTRIBUTING.md) — environment setup, testing, and code contribution workflow

> These docs replace the older [MAP Holons wiki](https://github.com/evomimic/map-holons/wiki) and provide a clearer
> separation between platform development and application usage.

## License

This project is licensed under
the [Cryptographic Autonomy License v1.0 (CAL-1.0)](https://opensource.org/licenses/CAL-1.0). This license ensures that
users of this software retain full control over their data and access to the underlying source code.

Note: This project uses the CAL-1.0 license with the Combined Work Exception to enable broader integration with larger
projects, while still protecting user autonomy and data access.

SPDX-License-Identifier: CAL-1.0

## Environment Setup

> PREREQUISITE: set up the [holochain development environment](https://developer.holochain.org/docs/install/).

Enter the nix shell by running this in the root folder of the repository:

```bash
nix develop
npm ci
```

**Run all the other instructions in this README from inside this nix shell, otherwise they won't work**.

`npm run deps:check` verifies that installed packages still match the root workspace manifests and lockfile. It runs
automatically before `npm start`. Script-only and descriptive manifest edits do not require reinstalling dependencies. `npm start` (and the `start:*` variants) also refuses to run outside `nix develop`;
set `MAP_SKIP_NIX_CHECK=1` to bypass that check if the toolchain is provided another way. If `deps:check` reports stale or missing dependencies, run `npm ci` from the repository root.
Use `npm install` only when intentionally changing npm dependencies and updating `package-lock.json`.

## Starting MAP

Welcome's native workflow and packaged page assets live in `host/crates/welcome`.
Conductora registers its commands and supplies access to the existing runtime,
startup state and configured storage base through a small native adapter.

Run `npm start` from the repository root. It builds the hApp and packaged debug
Conductora application, then launches packaged Welcome with the real persistent
conductor. This packaged engineering profile currently supports macOS on Apple
Silicon. Welcome first looks for the person's Space. A compatible existing PersonAgent
and Space lead to **Welcome back to MAP** and an explicit **Enter my space** action,
without repeating bootstrap or constitution. A fresh environment shows the invitation,
privacy and preparation story while its foundation loads. When ready, Welcome captures
the person's name through **Make this space mine**. Only after setup commits and the
saved association is verified does **You're on the MAP** offer **Enter my space**.
Preparation progress is indeterminate; **Setup details** exposes runtime stages and
elapsed time.

Expand **Space options** for **Reset and restart my Space**. Reset selects a fresh
environment and restarts MAP while keeping the previous environment and its data;
it does not copy the previous identity or content.

Personal setup stores the PersonAgent key in last/first/middle order, separated by
periods, and names the Space **First Middle Last's Personal Space**, omitting absent
optional names. The Space's key, `HolonSpaceName`, and `DisplayName` contain that
name. Setup creates a new version of the existing Space in the same transaction as
PersonAgent creation. Ownership belongs to the Space lineage, so its foundation
and other contents remain available through later Space versions.

Packaged MAP defers automatic local-gateway discovery and port mapping. Ordinary
startup retains peer/relay networking but does not opt into UPnP, PCP or NAT-PMP
probes. A native networked experience must explicitly opt in through
`irohTransport.enablePortMapping`. The macOS executable/display identity is MAP;
a local-network permission request includes a MAP-specific purpose description.

Integrity/DNA and coordinator compatibility are checked against the packaged hApp.
CoreSchema compatibility uses the existing schema-key check only. It does not compare
schema contents or perform migration. When incompatible, Welcome explains the mismatch
and offers **Create a new environment (keep existing)**. This explicit action selects
a separate keystore, conductor and recovery store and restarts MAP. The previous
environment is retained; its identities and content are not copied into the new one.
Ordinary startup errors do not offer environment replacement.

In packaged Welcome, expand **Saved environments** to review retained environments,
their creation dates and disk usage. Inactive environments offer **Reactivate and
restart**, which runs the normal compatibility checks on startup, or **Move to Trash**
after confirmation. Cleanup uses macOS Trash and never removes the active environment.
It does not migrate data between environments.

Packaged schema changes must advance the versioned release identity and its exact
schema dependency references, then regenerate the bootstrap and contribution
resources. Startup uses that new CoreSchema key to reject retained environments
from the previous release before Navigator opens. See the authoritative
[Core Schema release lifecycle](../map-dev-docs/docs/core/core-schema-bootstrap/core-schema-bootstrap-design-spec.md#schema-evolution).

`npm run start:info` and `npm run start:debug` use the same packaged lifecycle with
more logging. `npm run start:host` launches the last built package without rebuilding.
`npm run start:loader` remains an explicit development UI entrypoint.

## Running the backend tests

```bash
npm test
```

This runs unit checks and Sweettests. The Sweettest script builds the production hApp, builds the
loose `holons_test_probes` WASM, audits the packaged coordinator surface, and then runs the
conductor tests.

## MAP Schema CLI

The MAP schema authoring tool lives in `tools/map-schema` and is available through a repo-local npm wrapper:

```bash
npm run map-schema -- help
npm run map-schema -- check schema-src
npm run map-schema -- compile schema-src --out-dir generated/json-imports
```

Convenience scripts are also available for the core schema workflow:

```bash
npm run map-schema:check:coreschema
npm run map-schema:compile:coreschema
npm run map-schema:verify:coreschema
```

`map-schema:verify:coreschema` checks TDL syntax/lowering, verifies that the checked-in loader
JSON is fresh, and runs the JSON → TDL → JSON fidelity check using temporary output only.

The Rust crate already builds a `map-schema` binary target, but the repo does not require installing it globally yet. When we want a shell-native command later, the next step is `cargo install --path tools/map-schema`.

## Bootstrapping a network

Create a custom network of nodes connected to each other and their respective UIs with:

```bash
AGENTS=3 npm run network
```

Substitute the "3" for the number of nodes that you want to bootstrap in your network.
This will also bring up the Holochain Playground for advanced introspection of the conductors.

## Packaging

To package the web happ:

``` bash
npm run package
```

You'll have the `map-holons.webhapp` in `workdir`. This is what you should distribute so that the Holochain Launcher can
install it.
You will also have its subcomponent `map-holons.happ` in the same folder`.

## Documentation

This repository is using these tools:

- [NPM Workspaces](https://docs.npmjs.com/cli/v7/using-npm/workspaces/): npm v7's built-in monorepo capabilities.
- [hc](https://github.com/holochain/holochain/tree/develop/crates/hc): Holochain CLI to easily manage Holochain
  development instances.
- [@holochain/tryorama](https://www.npmjs.com/package/@holochain/tryorama): test framework.
- [@holochain/client](https://www.npmjs.com/package/@holochain/client): client library to connect to Holochain from the
  UI.
- [@holochain-playground/cli](https://www.npmjs.com/package/@holochain-playground/cli): introspection tooling to
  understand what's going on in the Holochain nodes.
