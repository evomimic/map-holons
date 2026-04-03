# PR #418 Summary — 412 Recovery and Transactional Snapshots

**Branch:** `412-crash-resilient-structural-undoredo-for-open-transactions`  
**Author:** @nphias  
**State:** Open (as of 2026-04-03)  
**Link:** https://github.com/evomimic/map-holons/pull/418

---

## What This PR Does

PR 418 implements crash-resilient structural undo/redo for open transactions. Key additions:

- **Recovery Store:** SQLite-backed persistence for transaction snapshots, created per receptor at startup.
- **Transaction Snapshots:** Undo/redo stacks per open transaction with checkpointing after successful commands.
- **`ClientSession` wrapper** on `client_context` for accessing the recovery store.
- **Dev-mode enhancements** (logging improvements, debug output) — extracted into separate PRs 422/429.
- **Storage configuration cleanup:** removed hardcoded defaults, fail-fast on missing config.
- **Provider abstraction:** moved provider-specific structs under `providers/` (IPFS, local).
- **Async performance** improvements for multi-provider scenarios.

> **Note:** This PR was preceded by two cleanup PRs:
> - **PR 422** — Dev mode + logging
> - **PR 429** — App-builder re-architected

---

## Definition of Done (Section 7 Status)

| Criterion | Status |
|---|---|
| Structural undo/redo stacks per open transaction | ✅ Done |
| Checkpoint after successful command completion | ✅ Done |
| Redo stack clears on new undoable command | ✅ Code done, ❌ not tested |
| `disable_undo` metadata behavior | ✅ Done |
| `snapshot_after` policy hook (mock acceptable) | ❌ Not implemented |
| SQLite schema (`recovery_session`, `recovery_checkpoint`, indexes) | ✅ Done |
| Snapshot blobs from wire serializer path | ✅ Done |
| Crash/restart restores consistent state + stacks | ✅ Code done, ⚠️ test doesn't verify content |
| Commit/rollback destroys history + deletes snapshot | ✅ Done (CASCADE) |
| Tests cover all areas | ⚠️ Partial |

---

## Key Review Discussion

### Reviewer Concerns (@evomimic)

1. **Architectural coupling:** Recovery persistence was coupled to provider setup (especially the Holochain provider path). The reviewer's position: recovery is an _IntegrationHub_ concern driven by transaction-staging responsibilities, not a Holochain-specific concern.

2. **SQLite not behind a receptor:** The reviewer expected recovery to be introduced behind a receptor abstraction, consistent with how the architecture treats other IntegrationHub/environment touch points.

3. **Two correctness bugs found and fixed:**
   - `transaction_store.rs`: `undo()` was restoring the current state rather than the previous state.
   - `app_builder.rs`: `create_window()` hardcoded `"holochain"` as the provider config key, but the config now uses named entries like `"holochain_dev"` / `"holochain_production"`.

### Author Response (@nphias)

- Receptors (by original design) are holonic network storage options; SQLite is a vendor choice, not a receptor.
- Recovery snapshots are available to **all** receptors/providers, not just Holochain.
- The `holons_recovery` crate is intentionally vendor-agnostic (SQLite can be swapped).
- Agreed to extract dev-mode/logging into separate PRs.
- Fixed both correctness bugs.
- Aligned on moving toward a `local_recovery` provider/receptor in a subsequent refactor.

### Architectural Alignment Reached

Both parties converged on:
- A **Receptor** = "a boundary-layer integration component that mediates interactions between the IntegrationHub and some external technology, service, or persistence substrate."
- Recovery persistence should be modeled as **its own provider-type** (not a per-provider flag), provisioned at app-builder setup time, currently backed by SQLite.
- PR 429 refactored this: snapshot recovery into its own `local_recovery` provider/receptor, available via the receptor factory by type or by ID.

---

## Gaps to Close (Outstanding)

| # | Gap | Priority |
|---|---|---|
| 1 | `snapshot_after` policy hook | Medium — add a stub trait / config flag |
| 2 | Redo-clearing test | Low — `persist A` → `undo A` → `persist B` → assert `can_redo() == false` |
| 3 | Blob roundtrip assertion | Medium — assert `snapshot.staged_holons` / `transient_holons` match original after undo/recover |
| 4 | Crash recovery simulation | Medium — persist → drop store → reopen → `recover_latest()` → assert content |
| 5 | Partial-write atomicity | Low — arguably covered by SQLite guarantees |

---

## Current Merge Guidance (@evomimic, latest)

> **Re-scope PR 418 to a minimal delta to enable merge before resolving PR 423 conflicts.**

Keep only:
1. `LocalConfig.features` with `#[serde(default)]`
2. `root_space` legacy handling change (non-panicking behavior)
3. Any tiny, directly-related compile fix required by the two items above

Defer to follow-up issues:
- Receptor architecture refactors
- Crate moves/splits (new receptor crates)
- Runtime/session rewiring
- Shared type package reshuffles
- Command-path refactors unrelated to 418

---

## Related PRs / Issues

| PR / Issue | Description |
|---|---|
| Issue #412 | Crash-resilient structural undo/redo for open transactions (parent issue) |
| PR #419 | Post-419 Runtime/MAP Commands architecture (conflict surface) |
| PR #422 | Dev mode + logging (extracted from 418, merged first) |
| PR #423 | Ongoing conflict surface with 418 |
| PR #429 | App-builder re-architected (extracted from 418) |
