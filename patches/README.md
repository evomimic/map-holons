# Local Crate Patches (root workspace)

This directory contains patched versions of upstream crates applied via `[patch.crates-io]` in the
root `Cargo.toml`. Each subdirectory is a full crate copy with a targeted fix.

The host workspace has its own, separate patch directory at
[`host/patches/`](../host/patches/README.md). The two are independent — a patch here does not
affect host builds, and vice versa.

---

## `portmapper` (v0.19.3) — disable UPnP / PCP / NAT-PMP by default

**Status: REQUIRED** for sweettests to run without emitting LAN NAT-discovery traffic.
Upstream: MIT OR Apache-2.0, <https://github.com/n0-computer/net-tools>.

### Patched file

`portmapper/src/lib.rs` — the `impl Default for Config` block. Three booleans and the accompanying default documentation; all other files match the published crate.

```rust
// upstream                         // patched
enable_upnp: true,                  enable_upnp: false,
enable_pcp: true,                   enable_pcp: false,
enable_nat_pmp: true,               enable_nat_pmp: false,
```

### The problem

Running `npm run sweetest` triggers an outbound-firewall prompt on every rebuild:

```
dance_tests-79fb8c75f9795bec wants to connect to 239.255.255.250 on UDP port 1900 (ssdp)
Code Signature: Not signed
```

That is UPnP discovery. The sweettest dependency chain is Holochain 0.7.1 →
Kitsune 0.5.2 → iroh 1.3.0 → portmapper 0.19.3.

1. `iroh-1.3.0/src/portmapper.rs` creates `portmapper::Client::default()` when its
   `PortmapperConfig` is enabled, which is the default.
2. `portmapper-0.19.3/src/lib.rs` enables all three discovery protocols in `Config::default()`.
3. `iroh-1.3.0/src/socket.rs` calls `procure_mapping()` during direct-address updates.

The single-agent sweettests do not need gateway port mapping. Their live loopback rendezvous
server provides the relay address Holochain needs for local agent registration, but does not
by itself disable gateway discovery.

### Why the patch remains necessary

Iroh now exposes both a `portmapper` Cargo feature and
`endpoint::Builder::portmapper_config(PortmapperConfig::Disabled)`. However,
`kitsune2_transport_iroh` 0.5.2 does not expose the latter through `IrohTransportConfig`,
and the resolved sweettest graph enables iroh's portmapper feature. Adding a direct dependency
with `default-features = false` cannot subtract features enabled by other dependencies.

Using the runtime setting requires changes to the transport. Until that setting reaches the
conductor configuration, this workspace-local patch preserves the test networking behavior
without changing production host networking.

### Why disabling the flags is sufficient

With all three false, the default client starts no UPnP, PCP, or NAT-PMP probes or mappings:

- `Probe::from_output` (lines 296 / 307 / 319) gates each probing task on
  `(enable_x && !x).then(..)`, so all three become `None`.
- The mapping selection in `Service::get_mapping` (lines 676 / 695 / 709) tests
  `upnp || self.config.enable_upnp`, `!recently_probed && self.config.enable_pcp`, and
  `.. enable_nat_pmp`. The `upnp` / `pcp` / `pmp` booleans come from a probe that never ran, so
  every branch is false and no mapping task spawns.

### Scope — read this before copying the patch to the host workspace

`[patch.crates-io]` is **unconditional**: it applies to every build in the workspace, and Cargo has
no way to make it depend on a profile, a feature, or a runtime flag such as `MAP_START_MODE=dev`.

It is applied **here only**, where that is harmless: the happ zomes compile to wasm and never pull
iroh, so `tests/sweetests` is the only thing in this workspace that reaches `portmapper`.

**Do not add it to `host/Cargo.toml`.** `conductora` is a real P2P application; disabling port
mapping there would degrade NAT traversal for *production* builds, not just dev mode, leaving
direct connections to fall back on the relay. Host dev mode needs a different mechanism.

### Removal criteria

Remove this patch once **either** of the following is true:

- Kitsune exposes iroh's `PortmapperConfig::Disabled` through conductor configuration and
  the sweettest harness selects it; **or**
- `kitsune2` exposes a transport that does not bind a wildcard UDP socket, making the whole
  question moot for sweettests.

When bumping `portmapper`, **re-vendor from the new version rather than editing in
place**, then re-apply the three-boolean change. After any bump, confirm the patch is still wired:
cargo must **not** warn `Patch 'portmapper ...' was not used in the crate graph`, and the
`Cargo.lock` entry for `portmapper` must have **no `source = "registry+..."` line** (patched crates
are listed with `dependencies` only).

Verify resolution without updating other dependencies:

```sh
cargo tree --manifest-path tests/sweetests/Cargo.toml --locked --offline -i portmapper
cargo tree --manifest-path host/Cargo.toml --locked --offline -i portmapper
```

The first must select `patches/portmapper`; the host must retain the registry crate.
