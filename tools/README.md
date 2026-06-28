# Tools

Standalone developer and CI utilities live under this directory.

## `map-schema`

`tools/map-schema` provides the MAP schema authoring CLI.

Useful commands:

```sh
cargo run --manifest-path tools/map-schema/Cargo.toml -- decompile host/import_files/map-schema/core-schema --out schema-src
npm run map-schema:decompile
```

The decompiler is intentionally separate from the `host/` workspace so it can be used as a standalone tool without linking into the IntegrationHub runtime.
