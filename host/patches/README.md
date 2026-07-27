# Local Crate Patches to Address Apple Silicon bug for holochain_wasmer_host

This directory contains patched versions of upstream crates applied via `[patch.crates-io]`
in `host/Cargo.toml`. Each subdirectory is a full crate copy with a targeted fix.

---

## Status — STILL REQUIRED (re-verified 2026-07-27 against Holochain 0.6.3)

**Upstream issue:** https://github.com/holochain/holochain-wasmer/issues/192 (open)

| Check | Result |
|---|---|
| `holochain` 0.6.1 / 0.6.2 / **0.6.3** dep on `holochain_wasmer_host` | **`=0.0.102` in all three** — the exact version patched here, so this patch applies unchanged |
| `holochain` 0.6.x dep on `wasmer` | `^6.0.1` throughout — unchanged |
| `holochain_wasmer_host` 0.0.103 (latest) `get_from_filesystem` | **still buggy** — byte-identical to 0.0.102 |
| `holochain/holochain-wasmer` `develop` branch | **still buggy** (`crates/host/src/module.rs:319`) |

No manifest change was needed for the 0.6.3 bump. After updating, confirm the patch is still wired:
cargo must **not** warn `Patch 'holochain_wasmer_host …' was not used in the crate graph`, and the
`host/Cargo.lock` entry for `holochain_wasmer_host` must have **no `source = "registry+…"` line**
(patched crates are listed with `dependencies` only).

Runtime check on 2026-07-27: a cold start on Apple Silicon at 0.6.3 compiled and wrote two fresh
entries to `/tmp/conductora_dev/<key>/wasm-cache/` with no crash — i.e. the `NotFound` → `Ok(None)`
path executed successfully. Note this confirms the patch **works**; it is not evidence the patch is
unnecessary. To test whether it is still load-bearing, comment out the `[patch.crates-io]` block,
`rm -rf /tmp/conductora_dev/*/wasm-cache`, and start the app — expect a silent SIGSEGV.

### Looking ahead to Holochain 0.7

`holochain` 0.7.0-rc.* moves to **`holochain_wasmer_host =0.0.103`** and **`wasmer =7.1.0`**. Both
halves of the bug survive that bump:

- 0.0.103's `get_from_filesystem` is unchanged from 0.0.102.
- `wasmer-compiler` 7.1.0 still captures a backtrace unconditionally on the user-error arm —
  `Trap::User(_err) => (wasm_trace(&info, None, &Backtrace::new_unresolved()), None)` in
  `src/engine/trap/stack.rs`. Its only new escape hatch is an `EXIT_CALLED` early return
  (wasmer #5877), which does not cover this path.

The patch directory must therefore be **re-vendored from 0.0.103, not edited in place** — the crate
was restructured:

| 0.0.102 | 0.0.103 |
|---|---|
| `src/module/wasmer_sys.rs`, `src/module/wasmer_wamr.rs` | `src/module/sys.rs`, `src/module/wasmi.rs` |
| `wasmer_sys`, `wasmer_sys_dev`, `wasmer_sys_prod`, `wasmer_wamr`, `error_as_host`, `debug_memory` | `wasmer-sys`, `wasmer-sys-cranelift`, `wasmer-sys-llvm`, `wasmer-wasmi`, `error-as-host`, `debug-memory` (underscores → hyphens; `wasmer_sys_dev` split into `wasmer-sys-cranelift`) |
| default = `["error_as_host", "wasmer_sys_dev"]` | default = `["error-as-host", "wasmer-sys", "wasmer-sys-cranelift"]` |

Re-apply the same `get_from_filesystem` rewrite to the vendored 0.0.103 source; the
`[patch.crates-io]` line itself needs no change once the vendored `version` reads `0.0.103`.

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

Filed 2026-07-27 as **https://github.com/holochain/holochain-wasmer/issues/192** (open):

> `ModuleCache::get_from_filesystem` turns a cache miss into a `RuntimeError`, causing a SIGSEGV on
> Apple Silicon. [BUG, MACOS]

Note it belongs to `holochain/holochain-wasmer`, not `holochain/holochain` — that is where
`module.rs` lives. The submitted text is kept at
[`.notes/holochain-wasmer-sigsegv-issue.md`](../../.notes/holochain-wasmer-sigsegv-issue.md).

**Remove this patch once that issue is fixed and a release carrying the fix is pinned by the
`holochain` version we depend on.**

### How the patch is wired

`host/Cargo.toml` contains:

```toml
[patch.crates-io]
holochain_wasmer_host = { path = "patches/holochain_wasmer_host" }
```

This applies only to the host-side build. Guest zomes (happ) are unaffected.
Remove this section once the fix lands upstream.
