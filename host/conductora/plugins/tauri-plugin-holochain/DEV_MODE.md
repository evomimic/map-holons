# Holochain Dev Mode

## Purpose

Normal production startup pays two costs:

1. **Lair KDF (~6–11 s on first run)** — `LairServerInProc` runs argon2 key
   derivation to unlock the persistent keystore.
2. **WASM compilation (~6 s on every restart)** — if the conductor data dir is
   wiped between runs, compiled WASM must be re-JIT'd from scratch each time.

Dev mode eliminates both: it uses Holochain's `DangerTestKeystore` (no KDF, no
lair process) and keeps the compiled WASM cache alive across restarts by preserving
`databases/wasm/`, `databases/db.key`, and `wasm-cache/` during the per-restart wipe.

It is intended **only for local single-user CRUD testing** where DHT persistence
across restarts is not required.

**Expected startup time in dev mode:**
- First (cold) run: ~10 s (WASM compiles once and is cached)
- Every subsequent run: **~3–4 s** (conductor DB migration + network init only)

---

## How to Enable

Add `"dev_mode": true` to the holochain provider in your storage config JSON
(typically `host/import_files/` or wherever your config JSON lives):

```json
{
  "holochain_dev": {
    "app_id": "map_holons",
    "bootstrap_url": "http://0.0.0.0:8888",
    "signal_url": "http://0.0.0.0:8080",
    "happ_path": "happ/workdir/map-holons.happ",
    "cell_details": [
      {
        "role_name": "map_holons",
        "zome_name": "holons",
        "zome_function": "dance"
      }
    ],
    "dev_mode": true,
    "enabled": true
  }
}
```

Setting `"dev_mode": false` (or omitting it entirely) restores normal production
behaviour. No other changes required.

---

## What Changes at Runtime

| Aspect | Production (`dev_mode: false`) | Dev mode (`dev_mode: true`) |
|---|---|---|
| Keystore | `LairServerInProc` — argon2 KDF, persisted keys | `DangerTestKeystore` — in-memory, no KDF |
| Device seed | Derived from `device_seed_lair_tag = "DEVICE_SEED"` | `danger_generate_throwaway_device_seed = true` |
| Conductor data dir | `{app_data_dir}/conductor/` (fully persistent) | `/tmp/conductora_dev` (selectively wiped each restart; WASM cache kept) |
| Agent key on install | `None` — conductor derives from device seed | Generated via `admin_ws.generate_agent_pub_key()` |
| Signal server | WAN SBD server (reachability check on startup) | Loopback SBD server on a random port (no WAN check) |
| mDNS peer discovery | Enabled | Disabled (not needed for single-node testing) |
| DHT / chain data | Persists across restarts | Wiped on every restart |
| Compiled WASM | Persists in production conductor dir | Preserved in `databases/wasm/` + `wasm-cache/` |

---

## Dev Restart Wipe Strategy

On each restart, `clean_dev_conductor_state` in `launch.rs` runs **before** the
conductor opens:

1. **Cache detection** — checks whether `databases/wasm/wasm` and `wasm-cache/`
   both exist from a previous run. Logs warm/cold start accordingly.

2. **Selective in-place delete** — walks the conductor dir and removes everything
   *except*:
   - `databases/db.key` — encryption key shared by all holochain DBs
   - `databases/wasm/` — WASM source/bytecode SQLite DB (encrypted)
   - `wasm-cache/` — pre-compiled native WASM modules (~20 MB)

   Deleted: `databases/conductor/`, `databases/cache/`, `databases/dht/`,
   `databases/authored/`, `databases/p2p/`, keystore dir, etc.

3. The conductor then starts with no installed apps and no stale agent records —
   but the WASM compile cache is intact, so `install_app` costs ~1 s instead of
   ~6 s.

**Why no WAL checkpoint?**
Holochain encrypts all its SQLite databases with `databases/db.key`. Plain
`rusqlite` cannot open these files ("unsupported file format"). Instead we
preserve the WAL files in-place and let holochain replay its own WAL on the next
open — which it already handles correctly.

**Why a hardcoded path (`/tmp/conductora_dev`) rather than `std::env::temp_dir()`:**
Inside Nix shells `TMPDIR` is session-specific (`/tmp/nix-shell.XXXXXX/`), so
`temp_dir()` returns a different path on every shell invocation. Using a fixed path
ensures the WASM cache survives across separate `nix-shell` sessions and
`npm run tauri dev` restarts.

---

## Log Signals

With `RUST_LOG=holochain_runtime=debug` (or the default `holochain_runtime=warn`)
you can watch these lines on restart:

| Log line | Meaning |
|---|---|
| `[LAUNCH] DEV MODE: wasm DB + wasm-cache found — warm start, WASM should be cached` | WASM cache preserved — next run will be fast |
| `[LAUNCH] DEV MODE: no existing wasm cache — cold start, WASM will compile` | First ever run / after `/tmp/conductora_dev` was deleted |
| `[LAUNCH] DEV MODE: conductor dir does not exist yet — cold start` | Very first run, conductor dir not yet created |
| `[LAUNCH] DEV MODE: conductor state reset in X.XXs (wasm cache preserved in-place)` | Wipe complete |
| `[HOLOCHAIN SETUP] App install/update done in ~1s` | WASM cache hit (warm start) |
| `[HOLOCHAIN SETUP] App install/update done in 6–7s` | WASM cache miss (cold start) |

---

## Files Changed

### 1. `holochain_runtime/src/config.rs`
**Added:** `pub dev_mode: bool` field (default `false`) and builder method `pub fn dev_mode(mut self) -> Self`.

```rust
pub struct HolochainRuntimeConfig {
    // ... existing fields ...
    /// Dev mode: skip lair, use ephemeral keystore. NOT for production.
    pub dev_mode: bool,
}

impl HolochainRuntimeConfig {
    pub fn dev_mode(mut self) -> Self {
        self.dev_mode = true;
        self
    }
}
```

`HolochainPluginConfig` is a type alias for `HolochainRuntimeConfig`, so the builder
method is available on plugin config too.

---

### 2. `holochain_runtime/src/launch/config.rs` — `conductor_config()`

**Added:** `dev_mode: bool` parameter. Branches on dev mode to set:
- `data_root_path` → `/tmp/conductora_dev` (hardcoded fixed path)
- `keystore` → `KeystoreConfig::DangerTestKeystore`
- `danger_generate_throwaway_device_seed = true`
- `network_config.signal_url` → local loopback SBD URL
- Advanced kitsune2 config: `signalAllowPlainText: true`, fast gossip intervals

Production path is unchanged (persistent `fs.conductor_dir()`, `LairServerInProc`,
`device_seed_lair_tag`).

```rust
if dev_mode {
    let dev_dir = std::path::PathBuf::from("/tmp/conductora_dev");
    config.data_root_path = Some(dev_dir.into());
    config.keystore = KeystoreConfig::DangerTestKeystore;
    config.danger_generate_throwaway_device_seed = true;
    // ... loopback signal URL + advanced gossip config ...
} else {
    config.data_root_path = Some(fs.conductor_dir().into());
    config.keystore = KeystoreConfig::LairServerInProc { lair_root: Some(lair_root) };
    config.device_seed_lair_tag = Some(DEVICE_SEED_LAIR_KEYSTORE_TAG.into());
}
```

---

### 3. `holochain_runtime/src/launch.rs` — `launch_holochain_runtime()`

**Three dev-mode branches:**

**a) Signal server** — skips the WAN reachability check (which can stall for 1–5 s)
and instead starts a local loopback SBD server immediately:

```rust
if config.dev_mode {
    let loopback = std::net::Ipv4Addr::LOCALHOST;
    let port = portpicker::pick_unused_port().expect("No ports free");
    let signal_handle = run_local_signal_service(loopback.to_string(), port).await?;
    maybe_local_signal_server = Some((url2!("ws://{loopback}:{port}"), signal_handle));
} else {
    // WAN check + optional fallback to LAN signal server
}
```

**b) Conductor build** — calls `clean_dev_conductor_state("/tmp/conductora_dev")`
(WAL checkpoint + selective wipe), then builds without lair:

```rust
let conductor_handle = if dev_mode {
    clean_dev_conductor_state(&dev_dir);
    Conductor::builder().config(conductor_config).build().await?
} else {
    let keystore = spawn_lair_keystore_in_proc(...).await?;
    // ... seed setup ...
    Conductor::builder().config(conductor_config).passphrase(Some(passphrase))
        .with_keystore(keystore).build().await?
};
```

**c) mDNS** — skipped in dev mode:

```rust
if dev_mode {
    tracing::info!("[LAUNCH] DEV MODE: skipping mDNS bootstrap");
} else {
    spawn_mdns_bootstrap(admin_port).await?;
}
```

**`clean_dev_conductor_state` function** (new, at bottom of `launch.rs`):
- Checks for `databases/wasm/wasm` + `wasm-cache/` (warm/cold detection)
- Walks conductor dir, removes everything except `databases/db.key`,
  `databases/wasm/`, and `wasm-cache/`
- Logs elapsed time and whether cache was found

---

### 4. `conductora/src/config/providers/holochain.rs`
**`HolochainConfig` struct:** added `pub dev_mode: Option<bool>` field.

**`holochain_plugin()` function:** reads `dev_mode` from `HolochainConfig` and calls
`.dev_mode()` on `HolochainPluginConfig` when `Some(true)`:

```rust
let mut plugin_config = HolochainPluginConfig::new(holochain_dir(&hc_cfg), ...);
if hc_cfg.dev_mode == Some(true) {
    plugin_config = plugin_config.dev_mode();
}
Ok(tauri_plugin_holochain::async_init(vec_to_locked(vec![]), plugin_config))
```

---

### 5. `conductora/src/setup/providers/holochain_setup.rs`

**Three-way branch in `setup()`** guards against `AppAlreadyInstalled` errors when
the selective wipe didn't run (e.g. the very first cold-start run when `wasm.db`
does not yet exist and `clean_dev_conductor_state` returns early):

```rust
if dev_mode && Self::is_app_installed(&installed_apps, app_id.clone()) {
    // Wipe was skipped (cold start edge case); app is already there — use it.
    tracing::warn!("Dev mode: app already installed, skipping update check.");
} else if dev_mode {
    // Normal dev path: conductor state wiped, install fresh with ephemeral key.
    Self::handle_new_app_installation(&handle, &admin_ws, happ, app_id, true).await?;
} else if Self::is_app_installed(&installed_apps, app_id.clone()) {
    Self::handle_existing_app(&handle, happ, app_id).await?;
} else {
    Self::handle_new_app_installation(&handle, &admin_ws, happ, app_id, false).await?;
}
```

**`handle_new_app_installation`** — added `admin_ws: &AdminWebsocket` and
`dev_mode: bool` parameters. In dev mode calls `generate_agent_pub_key()` explicitly
(required because `DangerTestKeystore` has no `device_seed_lair_tag` to derive from):

```rust
let agent_key: Option<AgentPubKey> = if dev_mode {
    Some(admin_ws.generate_agent_pub_key().await?)
} else {
    None // production: conductor derives key from device_seed_lair_tag
};
handle.holochain()?.install_app(app_id, happ, None, agent_key, None).await?;
```

---

## How to Reverse

To revert dev mode entirely (return to production-only behaviour):

1. **`holochain_runtime/src/config.rs`** — remove the `dev_mode` field and its
   builder method.
2. **`holochain_runtime/src/launch/config.rs`** — remove the `dev_mode` parameter
   from `conductor_config()`. Delete the `if dev_mode` block; keep only the
   production path (`fs.conductor_dir()`, `LairServerInProc`, `device_seed_lair_tag`).
3. **`holochain_runtime/src/launch.rs`** — remove the dev-mode signal branch,
   `clean_dev_conductor_state` call, and the mDNS skip. Collapse the conductor-build
   branch back to the single lair-based path. Delete the `clean_dev_conductor_state`
   function.
4. **`conductora/src/config/providers/holochain.rs`** — remove
   `pub dev_mode: Option<bool>` from `HolochainConfig`. Remove the
   `if hc_cfg.dev_mode == Some(true)` block in `holochain_plugin()`.
5. **`conductora/src/setup/providers/holochain_setup.rs`** — remove `admin_ws` and
   `dev_mode` parameters from `handle_new_app_installation()`. Revert `agent_key`
   to `None`. Collapse the three-way `setup()` branch back to the two-way production
   branch.
7. **Storage config JSON** — remove any `"dev_mode": true` entries.
