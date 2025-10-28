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
- 🛠️ [IDE Setup Guide](.dev/IDE_SETUP.md) — how to configure RustRover or VSCode for multi-workspace support
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
npm install
```

**Run all the other instructions in this README from inside this nix shell, otherwise they won't work**.

## Running 2 agents

```bash
npm start
```

This will create a network of 2 nodes connected to each other and their respective UIs.
It will also bring up the Holochain Playground for advanced introspection of the conductors.

## Running the backend tests

```bash
npm test
```

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
