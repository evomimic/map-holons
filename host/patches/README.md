# Local Crate Patches to Address Apple Silicon bug for holochain_wasmer_host

This directory contains patched versions of upstream crates applied via `[patch.crates-io]`
in `host/Cargo.toml`. Each subdirectory is a full crate copy with a targeted fix.

---

## `holochain_wasmer_host` (v0.0.102)

### Patched file
`holochain_wasmer_host/src/module.rs` — `ModuleCache::get_from_filesystem`

### The bug

On Apple Silicon (ARM64), starting the app after any coordinator or integrity zome change
causes a `SIGSEGV` and immediate process death. The crash is silent — no error message
is logged, no panic backtrace appears, `std::process::exit` is never reached.

The root cause is two bugs combining:

#### Bug 1 — `holochain_wasmer_host`: cache miss treated as fatal error

`ModuleCache::get_from_filesystem` opens a cache file by WASM hash. When the file does
not exist (`io::ErrorKind::NotFound`) it converts the `io::Error` into a
`wasmer::RuntimeError` instead of returning `Ok(None)` (cache miss):

```rust
// upstream code (broken)
let mut file = File::open(module_path).map_err(|e| {
    wasm_error!(WasmErrorInner::ModuleBuild(format!("{} Path: {}", e, module_path.display())))
})?;
```

The `?` operator triggers `From<WasmHostError> for wasmer::RuntimeError`, which calls
`wasmer::RuntimeError::user()`.

#### Bug 2 — ARM64 libunwind crash inside `RuntimeError::user()`

`RuntimeError::user()` always captures a backtrace:

```
wasmer::error::RuntimeError::user
  → wasmer_compiler::engine::trap::stack::get_trace_and_trapcode
    → backtrace::capture::Backtrace::new_unresolved
      → backtrace::backtrace::trace  (calls _Unwind_Backtrace)
        → libunwind::CFI_Parser::decodeFDE   ← SIGSEGV (null read at 0x0)
```

On Apple Silicon, `_Unwind_Backtrace` segfaults when it cannot parse the CFI (call frame
info) records for certain frames in the Holochain/wasmer/tokio stack. On Intel Macs and
Linux the backtrace capture succeeds and Bug 1 is silently swallowed (the error falls
through to WASM compilation). On M-series Macs it kills the process.

### When it triggers

The crash happens on the **first run after any change to a coordinator or integrity zome**
that produces a new WASM binary (new hash). The wasm-cache at
`/tmp/conductora_dev/<key>/wasm-cache/` has no entry for the new hash →
`get_from_filesystem` returns `NotFound` → process dies.

After a successful first install the compiled module is written to `wasm-cache/<hash>` and
subsequent runs with the same WASM hit the cache cleanly. Only hash changes (any source or
dep change that affects the binary) trigger the cold-start path.

### The fix (applied in this patch)

Treat **every** IO error in `get_from_filesystem` as a clean cache miss — never create a
`wasmer::RuntimeError` from this path:

```rust
// patched code
let module_path = match self.filesystem_module_path(key) {
    Some(p) => p,
    None => return Ok(None),
};
let mut file = match File::open(&module_path) {
    Ok(f) => f,
    Err(_) => return Ok(None),   // NotFound → cache miss, fall through to compilation
};
let mut bytes_mut = BytesMut::new().writer();
if std::io::copy(&mut file, &mut bytes_mut).is_err() {
    return Ok(None);
}
Ok(Some(bytes_mut.into_inner().freeze()))
```

The minimal upstream fix would be narrower — only swallow `NotFound`:

```rust
Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
```

But since any `RuntimeError` creation in this function risks the ARM64 crash, the patch
opts for the broader approach.

### Upstream report

This should be filed against the Holochain repository:
`https://github.com/holochain/holochain`

Include:
- The crash stack from `~/Library/Logs/DiagnosticReports/Conductora-*.ips`
- Repro: Apple Silicon, fresh/empty wasm-cache, any WASM install
- The one-line fix as suggested patch

### How the patch is wired

`host/Cargo.toml` contains:

```toml
[patch.crates-io]
holochain_wasmer_host = { path = "patches/holochain_wasmer_host" }
```

This applies only to the host-side build. Guest zomes (happ) are unaffected.
Remove this section once the fix lands upstream.
