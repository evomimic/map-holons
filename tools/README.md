# Tools

Standalone developer and CI utilities live under this directory.

## `map-schema`

`tools/map-schema` provides the MAP schema authoring CLI.

Useful commands:

```sh
cargo run --manifest-path tools/map-schema/Cargo.toml -- decompile host/import_files/map-schema/core-schema --out schema-src
cargo run --manifest-path tools/map-schema/Cargo.toml -- symbols host/import_files/map-schema/core-schema
npm run map-schema:compile
npm run map-schema:check
npm run map-schema:decompile
npm run map-schema:symbols
```

The decompiler is intentionally separate from the `host/` workspace so it can be used as a standalone tool without linking into the IntegrationHub runtime.
The `symbols` command prints the derived in-memory semantic symbol table for debugging; it is not a persisted source-of-truth artifact.
The `compile` command writes generated JSON imports to `generated/json-imports/` instead of overwriting the canonical inputs under `host/import_files/map-schema/core-schema/`.
The current baseline treats `schema-src/` and `generated/json-imports/` as paired golden trees, while `map-schema:check` is expected to render `no diagnostics` for the core corpus.
When comparing those trees in tests, the `meta` object is intentionally ignored because it is loader/tooling metadata rather than semantic schema content.
The canonical descriptor-aware IR now lives in [schema_ir.rs](map-schema/src/schema_ir.rs); [semantic.rs](map-schema/src/semantic.rs) remains as a compatibility re-export while the rest of the toolchain is migrated.
The derived lookup/index layer now lives in [schema_index.rs](map-schema/src/schema_index.rs); [symbols.rs](map-schema/src/symbols.rs) remains as a compatibility re-export for older callers.
The compiler backend now lowers shared Schema IR into [loader_ir.rs](map-schema/src/loader_ir.rs) before rendering canonical JSON, so the TDL compile path is no longer a direct descriptor-to-JSON emitter.
