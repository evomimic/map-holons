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
- `signal_url = wss://127.0.0.1:1`
- `relay_url = https://127.0.0.1:1`
- `target_arc_factor = 0`
- `advanced = None`

This prevents accidental WAN signal/relay usage in dev startup. All three URLs must be assigned
explicitly: `NetworkConfig::default()` carries real public servers (`dev-test-bootstrap2.holochain.org`
for bootstrap and signal, the iroh canary relay for relay), so any field left unset keeps its
public default.

`signal_url` and `relay_url` use TLS schemes (`wss`/`https`) rather than `ws`/`http` because
`advanced = None` means neither `signalAllowPlainText` nor `relayAllowPlainText` is set, and both
transports reject a plaintext URL without the matching flag. With TLS schemes, `advanced = None`
needs no companion advanced config — re-confirmed against iroh at holochain 0.6.3.

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
- `databases/wasm.db`
- `databases/wasm.db-wal`, `wasm.db-shm`, and `wasm.db-journal`, when present

Holochain 0.7 stores both WASM source and compiled modules in `wasm.db`. Its encryption key
and SQLite recovery files must stay together so pending writes can be recovered on restart.
The old `databases/wasm/` and `wasm-cache/` paths are no longer used and are removed during reset.

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

Conductora defaults `holochain::core::workflow::publish_dht_ops_workflow` to `error` in dev mode
to suppress repeated `NoPeersForLocation` warnings from an isolated conductor. This filters logs;
the publishing workflow still runs. Production uses the normal logging settings. To restore the
workflow's warnings explicitly, include its target in `RUST_LOG`:

```sh
RUST_LOG=host:warn,holochain::core::workflow::publish_dht_ops_workflow=warn npm run start:host
```

Key lines to watch:

- `Running in DEV MODE: using in-memory keystore and forcing local-only network config. NOT FOR PRODUCTION USE!`
- `[LAUNCH] DEV MODE: using persistent dev conductor dir ...`
- `[LAUNCH] DEV MODE: existing wasm.db found — preserving cached modules for reuse`
- `[LAUNCH] DEV MODE: no reusable wasm.db found — cold start, WASM will compile`
- `[LAUNCH] DEV MODE: conductor state reset in ...`
- `[LAUNCH] Total launch_holochain_runtime: ...`
- `[HOLOCHAIN SETUP] App install/update done in ...`


## Notes

- `holochain_websocket` warnings like `Close("None")` are commonly seen during admin/app websocket lifecycle transitions and are not, by themselves, evidence of WAN signal use.
- Dev mode intentionally prioritizes fast local iteration over production-like network behavior.
