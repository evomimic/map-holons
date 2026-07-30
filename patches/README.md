# Local Crate Patches (root workspace)

This directory contains patched versions of upstream crates applied via `[patch.crates-io]` in the
root `Cargo.toml`. Each subdirectory is a full crate copy with a targeted fix.

The host workspace has its own, separate patch directory at
[`host/patches/`](../host/patches/README.md). The two are independent — a patch here does not
affect host builds, and vice versa.

---

## `portmapper` (v0.12.0) — disable UPnP / PCP / NAT-PMP by default

**Status: REQUIRED** for sweettests to run without emitting LAN NAT-discovery traffic.
Upstream: MIT OR Apache-2.0, <https://github.com/n0-computer/net-tools>.

### Patched file

`portmapper/src/lib.rs` — the `impl Default for Config` block. Three booleans, nothing else.

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

That is UPnP discovery. It comes from iroh's port mapper, and **no configuration on our side can
turn it off**:

1. `iroh-holochain-0.95.1/src/magicsock.rs:1803` builds the client as
   `portmapper::Client::with_metrics(Default::default(), ..)` — the config is hardcoded to the
   crate default, with no plumbing to override it.
2. `portmapper-0.12.0/src/lib.rs:130-139` — that default enables all three protocols.
3. `magicsock.rs:1292` calls `procure_mapping()` at the top of every direct-address update,
   **before** the `if self.relay_map.is_empty() { return }` early-out on line 1299. So it fires
   regardless of what `bootstrap_url` / `signal_url` / `relay_url` are set to.

A single in-process sweettest conductor never peers, so there is nothing for NAT traversal to
traverse. The dependency is accidental, not designed.

### Why the alternatives do not work

| Alternative | Result |
|---|---|
| Point every WAN endpoint at `127.0.0.1:1` | Done — see `mock_conductor.rs::local_only_config`. Removes WAN dialing and cuts socket churn ~26x, but does **not** stop SSDP: `procure_mapping()` runs before the relay-map check. |
| Turn off a cargo feature | `portmapper` is a **non-optional** dependency of `iroh` (no `optional = true`), and none of iroh's features gate it. |
| A config knob | `IrohTransportConfig` exposes only `relay_url`, `relay_allow_plain_text`, `max_frame_bytes`, `connect_timeout_s` and auth material. `NetworkConfig`'s `test-utils` flags (`disable_bootstrap` / `disable_publish` / `disable_gossip`) gate kitsune modules, not the transport. |
| Drop the `transport-iroh` feature from sweettests | **Does not compile.** `kitsune2-0.4.1/src/lib.rs:49` — `error[E0063]: missing field 'transport' in initializer of 'kitsune2_api::Builder'`. Only one of the two transport cfgs initialises the field, there is no mem-transport feature, and `holochain_p2p/src/spawn/actor.rs:504` calls `kitsune2::default_builder()` internally with no injection hook. |

### Why disabling the flags is sufficient

With all three false, no packet is sent — not merely fewer:

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

- `iroh` accepts a `portmapper::Config` (or an equivalent "disable port mapping" switch) through
  its endpoint builder, and the version we depend on carries it; **or**
- `kitsune2` exposes a transport that does not bind a wildcard UDP socket, making the whole
  question moot for sweettests.

When bumping `portmapper` past 0.12.0, **re-vendor from the new version rather than editing in
place**, then re-apply the three-boolean change. After any bump, confirm the patch is still wired:
cargo must **not** warn `Patch 'portmapper ...' was not used in the crate graph`, and the
`Cargo.lock` entry for `portmapper` must have **no `source = "registry+..."` line** (patched crates
are listed with `dependencies` only).
