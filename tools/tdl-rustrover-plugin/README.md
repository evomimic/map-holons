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

The plugin contains no TDL parser or semantic model. RustRover receives diagnostics,
document symbols, definitions, usages, and hover text exclusively from the Rust
editor service.
