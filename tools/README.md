# Tools

Standalone developer and CI utilities live under this directory.

## `happ-artifact-audit`

`tools/happ-artifact-audit` verifies that packed production DNAs and hApps contain exactly the
approved coordinator zomes and that each packaged coordinator WASM exactly matches
`happ/coordinator-surface.toml`. It reads the WASM from the bundle being shipped, rather than from
a presumed Cargo target directory, so packaging drift cannot be hidden by a correct loose build.

Repo-local usage supplies every artifact path explicitly:

```sh
npm run check:happ-artifacts
```

The root `check:happ-artifacts` script supplies `--probe-wasm` for the loose
`holons_test_probes` test artifact and enables `--deny-production-test-only`, so CI rejects any
test-only zome-call export that appears in the packaged production coordinator surface.

## `map-schema`

`tools/map-schema` provides the MAP schema authoring CLI. It is already a Rust
binary target named `map-schema`; for now, repo-local usage goes through npm so
contributors do not need to install the binary globally.

Repo-local CLI usage:

```sh
npm run map-schema -- help
npm run map-schema -- check schema-src
npm run map-schema -- compile schema-src --out-dir generated/json-imports
npm run map-schema -- roundtrip-json generated/json-imports --tdl-out generated/tdl-decompiled --json-out generated/json-roundtrip
```

Core schema convenience commands:

```sh
npm run map-schema:check:coreschema
npm run map-schema:compile:coreschema
npm run map-schema:roundtrip:coreschema
```

Direct Cargo usage remains available when you want to bypass npm:

```sh
cargo run --manifest-path tools/map-schema/Cargo.toml -- help
cargo run --manifest-path tools/map-schema/Cargo.toml -- compile schema-src --out-dir generated/json-imports
```

Later, when we want a shell-native command, this crate is ready for local installation:

```sh
cargo install --path tools/map-schema
map-schema help
```

`schema-src/` is the editable Schema 2.0 source corpus. `generated/json-imports/` is the checked-in
loader artifact produced from that source. The decompiler is intentionally separate from the
`host/` workspace so it can be used as a standalone tool without linking into the IntegrationHub
runtime. Source tooling parses, lowers, renders, and compares loader facts; descriptor semantics
remain owned by Holons Core and Descriptor-Aware Holon Validation.
