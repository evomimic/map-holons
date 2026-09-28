# MAP TDL for RustRover

This IntelliJ Platform plugin associates `.tdl` files with MAP TDL and starts the
source-only editor service from this repository:

    map-schema lsp

Build the Rust executable before launching RustRover:

    cargo build --manifest-path tools/map-schema/Cargo.toml

The plugin discovers the standard debug executable at
`tools/map-schema/target/debug/map-schema` in the opened repository. Otherwise, ensure
`map-schema` is on `PATH`, or set the JVM system property `map.tdl.lsp.executable` to
its absolute path. The server indexes available source files only. It deliberately does
not query the DHT, resolve saved-holon keys, or run descriptor-aware validation.

The plugin has a presentation lexer and a flat PSI parser for IntelliJ file lifecycle,
syntax highlighting, and Structure view integration. These do not recognize semantic
declarations or build an independent reference index. The Structure adapter consumes
`map-schema editor-outline`; navigation, usages, hover, and diagnostics consume the
compiler-backed LSP. Both APIs derive from the same Rust parser/lowering result and
bounded source-provenance sidecar.

The server also exposes workspace symbols and the source-only authored relationship
graph through `tdl/relationshipGraph`. This plugin does not provide a graph visualizer.
See [the editor-service API](../map-schema/README.md) for the protocol and recovery
contract.

Build the plugin with `./gradlew buildPlugin` from this directory. In RustRover, verify
Structure and Go to Definition on `schema-src/core/root.tdl`, cross-file usages, and an
incomplete declaration buffer. Incomplete declarations remain in the outline, while
only successfully lowered declarations contribute graph facts.
