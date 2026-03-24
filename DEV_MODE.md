# Holochain Dev Mode

## Purpose

Dev mode is a local-development runtime profile optimized for fast restart loops.

It is not production-like and should only be used for local CRUD/testing workflows.

Core goals:

1. Remove lair/key-derivation startup cost.
2. Preserve Holochain WASM cache across restarts.
3. Reset app/conductor state each run so dev starts are reproducible.

## Activation

Dev mode is controlled by one launch-time environment variable:

```sh
HC_DEV_MODE=1
```

Truthy values are: `1`, `true`, `yes`, `on` (case-insensitive).

When `HC_DEV_MODE` is unset (or falsey), runtime uses normal mode.

## Runtime Behavior

| Area | Normal Mode | Dev Mode (`HC_DEV_MODE=1`) |
|---|---|---|
| Keystore | `LairServerInProc` | `DangerTestKeystore` |
| Device seed | `device_seed_lair_tag = "DEVICE_SEED"` | `danger_generate_throwaway_device_seed = true` |
| Conductor data root | `fs.conductor_dir()` | derived dev dir under `/tmp/conductora_dev/<hash>` |
| Signal setup at launch | policy-driven (see below) | skipped entirely (no WAN reachability check, no local signal service launch) |
| mDNS bootstrap | enabled | skipped |
| Persistent chain/app state | persisted | wiped each launch (WASM cache preserved) |

## Signal Policy

### Dev mode

At launch, dev mode does not run signal setup:

- no WAN signal host reachability check
- no local signal server startup

Conductor network config is forced to local-only placeholders:

- `signal_url = ws://127.0.0.1:1`
- `bootstrap_url = http://127.0.0.1:1`
- `target_arc_factor = 0`

This prevents accidental WAN signal usage in dev startup.

### Normal mode

Normal mode uses `signal_url_configured` intent from storage config:

- `signal_url` missing/null: attempt local signal server startup first.
- `signal_url` configured: run WAN reachability check; if unreachable and `fallback_to_lan_only=true`, attempt local signal fallback.

## Dev Data Directory Isolation

Dev data root is deterministic and derived from:

- provider name
- app id
- canonical workspace path

The helper `dev_conductor_dir(provider_name, app_id)` hashes this input and writes to:

```text
/tmp/conductora_dev/<hash>
```

This avoids a single shared global dev conductor path across clones/branches.

## Restart Wipe Strategy (Cache-Preserving)

Before conductor build in dev mode, `clean_dev_conductor_state(...)` runs.

Preserved:

- `databases/db.key`
- `databases/wasm/`
- `wasm-cache/`

Deleted:

- remaining DB/state paths (conductor/cache/dht/authored/p2p/etc.)
- other conductor state outside preserved cache artifacts

Result:

- fresh app/conductor state each run
- warm WASM cache retained for faster follow-up starts

## Expected Timing

Typical dev warm run is split into phases:

1. Conductor launch: usually around `~3-4s`.
2. Holochain setup/install path: usually around `~1.0-1.5s`.
3. UI readiness can appear later due to frontend retry backoff.

If warm start regresses, check logs first before assuming cache loss.

## Useful Logs

Set host logging to include launch debug logs:

```sh
RUST_LOG=info
# or for more detail:
RUST_LOG=holochain_runtime=debug,conductora_lib=debug
```

Key lines to watch:

- `HOLOCHAIN DEV MODE ENABLED: ...`
- `[LAUNCH] DEV MODE: skipping all signal setup ...`
- `[LAUNCH] DEV MODE: wasm DB + wasm-cache found — warm start ...`
- `[LAUNCH] DEV MODE: conductor state reset in ...`
- `[LAUNCH] Conductor ready in ...`
- `[HOLOCHAIN SETUP] App install/update done in ...`

## Notes

- `holochain_websocket` warnings like `Close("None")` are commonly seen during admin/app websocket lifecycle transitions and are not, by themselves, evidence of WAN signal use.
- Dev mode intentionally prioritizes fast local iteration over production-like network behavior.
