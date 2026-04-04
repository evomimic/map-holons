# Issue #436 — Chain-of-Command Restructure for Receptors

**Branch:** `436-chain-of-command-restructure-for-receptors`  
**Key Commit:** [`e3052d0`](https://github.com/evomimic/map-holons/commit/e3052d0ff6b112afe6e988e35c0a13514d985bd7) — *"integration work for the recovery receptor"*  
**Issue:** https://github.com/evomimic/map-holons/issues/436  
**Builds on:** PR #418 (recovery and transactional snapshots)

---

## 1. Problem Statement (Issue #436)

The move to implement the MAP command spec with the runtime/session work introduced an **inversion of control** at the receptor layer.

**Old model:** Receptors were self-contained units that implemented `ReceptorBehaviour`, built their own configuration, and routed requests themselves. The `holons_client` crate consumed them passively.

**New model:** The Runtime layer determines which receptors to use. The `holons_client` crate is responsible for receptor production, caching, and routing. Each receptor is its own crate with its own storage/network configuration. `ReceptorBehaviour` is removed and replaced by a unified `Receptor` enum.

This work transcends and includes the PR #418 recovery feature.

---

## 2. Architectural Changes

### 2.1 Chain-of-Command (Before → After)

**Before:**
```
TS Client → Receptor (builds config, routes, handles requests) → holons_client → Runtime
```

**After:**
```
Runtime (determines receptors needed)
    → holons_client (ReceptorFactory + ReceptorCache)
        → Receptor enum (dispatches to concrete implementation)
            → External system (Holochain Conductor / SQLite / etc.)
```

The Runtime now **selects** receptors through `ReceptorFactory`. The `holons_client` is the owner of receptor lifecycle. Each concrete receptor type handles only its own concerns.

### 2.2 `ReceptorBehaviour` Removed → `Receptor` Enum

The `ReceptorBehaviour` trait was removed. In its place, `holons_client/src/lib.rs` exposes a `Receptor` enum:

```rust
pub enum Receptor {
    Holochain(HolochainReceptor),
    LocalRecovery(LocalRecoveryReceptor),
}
```

Methods like `handle_map_request`, `get_space_info`, and `transaction_context` are dispatched through this enum. New receptor variants are added to the enum rather than implementing a shared trait.

> **Note:** `Local(LocalReceptor)` is present in the enum but currently disabled (see §3.3).

### 2.3 `ReceptorType` as the Shared Identity

`ReceptorType` (in the new `shared_types` crate) is the canonical identifier for a receptor's role:

```rust
pub enum ReceptorType {
    Local,
    LocalRecovery,
    Holochain,
}
```

`BaseReceptor` carries a `receptor_type`, a `receptor_id` (derived from the name in `storage.json`), optional `client_handler`, and `properties`. This is the configuration contract between setup and the factory.

---

## 3. New Crates Introduced

### 3.1 `recovery_receptor` (`host/crates/recovery_receptor/`)

**Purpose:** A self-contained receptor for crash-resilient transaction snapshot persistence, backed by SQLite. This is the outcome of the architectural discussion in PR #418 — recovery is now its own receptor, not a flag on the Holochain receptor.

**Key types:**

| Type | File | Responsibility |
|---|---|---|
| `LocalRecoveryReceptor` | `local_recovery_receptor.rs` | Receptor impl — init session, persist snapshots, undo/redo |
| `TransactionRecoveryStore` | `storage/transaction_store.rs` | SQLite-backed store; two tables: `recovery_session` + `recovery_checkpoint` |
| `TransactionSnapshot` | `storage/transaction_snapshot.rs` | Serializable snapshot of staged/transient holons + undo/redo stacks |
| `RecoveryStore` (trait) | `storage/recovery_store.rs` | Abstract persistence interface |

**`LocalRecoveryReceptor` lifecycle:**

1. Created by `ReceptorFactory` when a `BaseReceptor` of type `LocalRecovery` is registered.
2. `init_session(context)` — called once during runtime init. Attempts crash recovery by checking for orphaned transactions in the store.
3. `persist(description, disable_undo)` — called after every successful command to checkpoint the transaction state.
4. `undo()` / `redo()` — restore prior/forward checkpoints.

**SQLite schema (embedded, no migration files):**

```sql
-- One row per open transaction
CREATE TABLE recovery_session (
    tx_id                 TEXT PRIMARY KEY,
    lifecycle_state       TEXT NOT NULL DEFAULT 'Open',
    latest_checkpoint_id  TEXT,
    undo_stack_json       TEXT NOT NULL DEFAULT '[]',
    redo_stack_json       TEXT NOT NULL DEFAULT '[]',
    format_version        INTEGER NOT NULL DEFAULT 1,
    updated_at_ms         INTEGER NOT NULL
);

-- One row per undo/redo checkpoint
CREATE TABLE recovery_checkpoint (
    checkpoint_id  TEXT PRIMARY KEY,
    tx_id          TEXT NOT NULL,
    stack_kind     TEXT NOT NULL CHECK (stack_kind IN ('undo', 'redo')),
    stack_pos      INTEGER NOT NULL,
    snapshot_blob  BLOB NOT NULL,
    ...
    FOREIGN KEY (tx_id) REFERENCES recovery_session(tx_id) ON DELETE CASCADE
);
```

Commit/rollback deletes the entire transaction via `CASCADE`.

---

### 3.2 `shared_types` (`host/crates/shared_types/`)

**Purpose:** Extracts host-side shared type definitions out of `holons_client/src/shared_types/` into a standalone crate, reducing coupling.

**Exports:**

| Module | Contents |
|---|---|
| `base_receptor` | `BaseReceptor`, `ReceptorType` |
| `map_request` | `MapRequest` |
| `map_response` | `MapResponse` |
| `holon_space` | `HolonSpace`, `SpaceInfo` |

This crate is re-exported as `client_shared_types` in workspace members.

---

## 4. Crate Renames

| Old Name | New Name | Reason |
|---|---|---|
| `holochain_receptor` | `deprecated_holochain_receptor` | The Holochain receptor path is being superseded by the MAP Commands / Runtime architecture. The crate is kept for compatibility during transition. |
| `holons_receptor` | `local_receptors` | Renamed to reflect its actual scope. The factory and cache that were here are now in `holons_client`. |

> **`local_receptors` is currently unimplemented.** The `lib.rs` stubs out the public API with `//unimplemented` comments. `LocalReceptor` exists as a type but its setup is disabled.

---

## 5. Changes to `holons_client`

`holons_client` is now the **owner of receptor lifecycle**.

### New files:

| File | Purpose |
|---|---|
| `receptor_factory.rs` | `ReceptorFactory` — creates receptors from `BaseReceptor` configs, registers them in the cache. Moved from `holons_receptor`. |
| `receptor_cache.rs` | `ReceptorCache` — thread-safe `Arc<Mutex<HashMap<ReceptorKey, Arc<Receptor>>>>`. Look up by type or by ID. |
| `client_session.rs` | `ClientSession` — wraps a `HolonSpaceManager` + optional `LocalRecoveryReceptor`. Opens (or recovers) a transaction on construction. |

### `ReceptorFactory` API:

```rust
// Retrieve all receptors of a given type
fn get_receptors_by_type(receptor_type: &ReceptorType) -> Result<Vec<Arc<Receptor>>, HolonError>

// Get the first (default) receptor of a type
fn get_default_receptor_by_type(receptor_type: &ReceptorType) -> Result<Arc<Receptor>, HolonError>

// Get a receptor by its config name (receptor_id)
fn get_receptor_by_id(receptor_id: &String) -> Result<Arc<Receptor>, HolonError>

// Load and register receptors from BaseReceptor configs (called during setup)
async fn load_from_configs(configs: Vec<BaseReceptor>) -> Result<(), ...>
```

### `ClientSession` construction:

```rust
ClientSession::new(space_manager, recovery: Option<Arc<Receptor>>, destination: Option<Arc<Receptor>>)
```

On creation:
- If `recovery` is `Some(LocalRecovery(...))` and the store has orphaned transactions from a prior crash → reopens that transaction and restores its snapshot.
- Otherwise → opens a fresh transaction and initialises the recovery receptor with it.

---

## 6. Changes to `conductora`

### `setup/providers/local/setup.rs`

`LocalSetup::setup()` now checks whether the `"recovery"` feature is listed in `LocalConfig.features`. If so, it creates a `TransactionRecoveryStore` (async, offloaded via `spawn_blocking`) and registers a `BaseReceptor` of type `LocalRecovery`.

```
storage.json → LocalConfig { features: ["recovery"] }
    → LocalSetup::build_recovery_receptor()
        → create_snapshot_store()     // creates SQLite DB at {app_data_dir}/storage/{name}/snapshots.db
        → BaseReceptor { type: LocalRecovery, handler: Arc<TransactionRecoveryStore> }
        → register_receptor(handle, base_receptor)  // stored in Tauri app state as ReceptorFactory
```

### `runtime/init_runtime.rs`

`init_from_state()` now:

1. Initialises the `HolonSpaceManager` via `init_client_runtime(Some(initiator))`.
2. **Retrieves the `LocalRecoveryReceptor`** from the `ReceptorFactory` in Tauri app state.
3. Creates a `ClientSession` with the space manager and recovery receptor.
4. Registers a `RuntimeSession` and `Runtime` into Tauri app state.

```rust
fn get_recovery_receptor_from_factory(handle: &AppHandle) -> Option<Arc<Receptor>> {
    handle.try_state::<ReceptorFactory>()
        .and_then(|factory| factory.get_default_receptor_by_type(&ReceptorType::LocalRecovery).ok())
}
```

If no recovery receptor is registered (e.g., the `"recovery"` feature is disabled in config), recovery is silently disabled.

### `config/storage.json`

A new `"local_recovery"` provider entry is registered:

```json
"local_recovery": {
  "type": "local",
  "data_dir": "./data/local_storage",
  "features": ["recovery"],
  "enabled": true
}
```

This is what triggers `LocalSetup` to create the `LocalRecoveryReceptor` at startup.

### `setup/receptor_config_registry.rs`

Updated to handle the new receptor types and IDs from the refactored provider setup. Storage provider entries with the same name (which becomes `receptor_id`) now cause a startup failure, enforcing uniqueness.

---

## 7. Crate Dependency Graph (Post-Commit)

```
conductora
 ├── holons_client          (ReceptorFactory, ReceptorCache, ClientSession, Receptor enum)
 │    ├── shared_types      (BaseReceptor, ReceptorType, MapRequest/Response)
 │    ├── recovery_receptor (LocalRecoveryReceptor, TransactionRecoveryStore)
 │    └── deprecated_holochain_receptor  (HolochainReceptor — transitional)
 ├── map_commands           (Runtime, RuntimeSession)
 ├── local_receptors        (LocalReceptor — stub/unimplemented)
 └── shared_types
```

`recovery_receptor` has **no dependency on Holochain**. It depends only on `core_types`, `holons_core`, and `rusqlite`.

---

## 8. What Is Still Pending

| Item | Notes |
|---|---|
| `LocalReceptor` implementation | `local_receptors` crate exists but is fully stubbed out |
| `ClientSession` methods | `commit`, `undo`, `redo`, `add`, `save`, `list` are noted as TODOs |
| `get_root_spaces()` | Returns `NotImplemented` |
| MAP Commands integration | `ClientSession` is created but not yet wired into command dispatch |
| `snapshot_after` policy hook | Deferred from PR #418; still not implemented |
| Redo-clearing test | `can_redo() == false` after new command following undo |
| Blob roundtrip assertion | Assert snapshot content survives undo/recover round-trip |

---

## 9. Key Principles Established

1. **Recovery persistence is its own receptor.** It is not a flag on the Holochain receptor or any other primary storage receptor. It is configured independently in `storage.json` and provisioned via its own provider path.

2. **`ReceptorType` drives lookup, not string matching.** The factory resolves receptors by `ReceptorType` enum, eliminating the hardcoded `"holochain"` string-key lookup bug fixed in PR #418.

3. **`storage.json` provider names are source of truth for `receptor_id`.** Duplicate names cause a startup failure. The name becomes the receptor's canonical ID in the factory cache.

4. **Blocking I/O stays off the async thread.** SQLite open and directory creation are wrapped in `tokio::task::spawn_blocking`.

5. **Crash recovery is automatic.** On startup, `ClientSession::new` checks the recovery store for orphaned transactions. If found, the session reopens the transaction and restores the last snapshot before returning to the caller.
