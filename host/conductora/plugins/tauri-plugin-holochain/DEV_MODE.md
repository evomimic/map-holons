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
MAP_START_MODE=dev
```

When `MAP_START_MODE` is unset or set to `prod`, runtime uses normal mode.

## Runtime Behavior

| Area | Normal Mode | Dev Mode (`MAP_START_MODE=dev`) |
|---|---|---|
| Keystore | `LairServerInProc` (in-process lair) | `DangerTestKeystore` (in-memory, ephemeral) |
| Device seed | Created in lair under tag `"DEVICE_SEED"` | Not used; setup generates an explicit agent key via `generate_agent_pub_key()` |
| Conductor data root | `fs.conductor_dir()` | derived dev dir under `/tmp/conductora_dev/<hash>` |
| mDNS bootstrap | enabled | skipped |
| Persistent chain/app state | persisted | wiped each launch (WASM cache preserved) |

## Network Config

### Dev mode

Conductor network config is forced to local-only placeholders (from `launch/config.rs`):

- `bootstrap_url = http://127.0.0.1:1`
- `relay_url = https://127.0.0.1:1`
- `target_arc_factor = 0`
- `advanced = None`

This prevents accidental WAN signal/relay usage in dev startup.

### Normal mode

Normal mode applies an `advanced` JSON config enabling plaintext for both transports and tuning reSign intervals:

```json
{
  "tx5Transport":  { "signalAllowPlainText": true },
  "irohTransport": { "relayAllowPlainText": true, "coreBootstrap": { "backoffMaxMs": 20000 } },
  "coreSpace":     { "reSignExpireTimeMs": 20000, "reSignFreqMs": 20000 }
}
```

`bootstrap_url` and `relay_url` are taken from the storage config (`HolochainConfig`). If neither is configured, network defaults apply.

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

- `Running in DEV MODE: using in-memory keystore and forcing local-only network config. NOT FOR PRODUCTION USE!`
- `[LAUNCH] DEV MODE: using persistent dev conductor dir ...`
- `[LAUNCH] DEV MODE: wasm DB + wasm-cache found — warm start ...`
- `[LAUNCH] DEV MODE: no existing wasm cache — cold start, WASM will compile`
- `[LAUNCH] DEV MODE: conductor state reset in ...`
- `[LAUNCH] Total launch_holochain_runtime: ...`
- `[HOLOCHAIN SETUP] App install/update done in ...`

## Notes

- `holochain_websocket` warnings like `Close("None")` are commonly seen during admin/app websocket lifecycle transitions and are not, by themselves, evidence of WAN signal use.
- Dev mode intentionally prioritizes fast local iteration over production-like network behavior.
