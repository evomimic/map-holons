# Native dependency patches

These native patches are explicitly selected by `host/Cargo.toml`. The root
workspace also selects the same three Holochain crates for Sweettest coverage of
the embedded-conductor backport. They do not change hApp/WASM dependencies.
The root-only `portmapper` patch is not applied to the production host.

## `holochain_data` 0.7.1 — native queries and credential wrapping

Source: published `holochain_data` 0.7.1, upstream Holochain commit
`c5f5028274c0406cb9d88d0fa48080135137fd75`, `crates/holochain_data`.
The published crate metadata declares Apache-2.0; that declaration is retained.
`LICENSE-APACHE` adds the standard license text from apache.org (the published
crate contains no license file). Registry checksum:
`5d302613e1a2a5ec13a71f1a8af37b019b0918908138dbdce8de57c0bbe86611`.

Local changes are limited to:

- An additive, non-unique DHT location-index migration for validated ChainOps and
  integrated WarrantOps. Existing migrations and checksums are unchanged.
- Concrete wrapping/non-wrapping predicates for `op_hashes_in_time_slice` and
  `earliest_authored_timestamp_in_arc`. Arc bounds remain bound parameters; the
  SQL contains only fixed table names. SQLite can range-search location indexes
  instead of scanning the whole index for each Kitsune sector.
- `DbKey::rewrap`, an in-memory passphrase rewrapping operation that preserves the
  existing SQLCipher key and salt. It generates fresh wrapping ciphertext without
  writing a database or changing its encryption key. The native credential
  transition adapter uses it alongside the matching Lair 0.7.1 wrapper adapter.
- Source-chain action-type/sequence SQL prefilters from the init backport below.
- Regression tests for query plans, inclusive/wrapping/singleton arcs, half-open
  time bounds, warrants and private/unvalidated-op filtering.

The result set, privacy filters, gossip participation, keys, identities, operation
validation and Commit semantics are unchanged. The source-chain prefilter is described separately below; other queries are unmodified.
This addresses conductor cell initialization before MAP schema loading; it does
not change or bypass initial schema bootstrap.

### Evidence

On a disposable copy of a retained conductor store, all 512 earliest-timestamp
sector reads took 209.94 seconds before the patch and 0.732 seconds afterward,
with identical timestamp/count aggregates. A separate single-connection comparison
with `PRAGMA busy_timeout=0` returned identical results for eight sectors: original
predicate 2.578 seconds, range predicate 0.014 seconds; connection acquisition
0.006 seconds. This distinguishes query execution cost from lock waiting in the
isolated reproduction. Indexes alone were insufficient: the original guarded-OR
predicate still scanned them.

All 146 upstream/local library tests (including 11 sync-query tests) passed in an
isolated copy with `test-utils,encryption`, using
the existing native lockfile and omitting only the unused Criterion benchmark
dependency from that temporary manifest. No runtime dependency versions changed.
The host's real-conductor lifecycle test exercises the patched dependency through
the application startup orchestration. Live packaged evidence is tracked in the
Agent Spaces implementation plan in map-dev-docs.

Credential-wrapper tests in `holochain_runtime::credential_custody` exercise a
disposable encrypted Conductor database and Lair seed store. They verify unchanged
content and signing identity after rewrapping, rejection of the old passphrase,
and no writes during transition preparation. This does not implement a live
two-file cutover or demonstrate app-private Keychain access. The wrapper adapter
must be reviewed against upstream format/KDF changes on dependency upgrades.

### Removal and upgrades

Keep this patch bounded and seek an upstream equivalent. Do not remove the patch
merely because upstream adds a location index: verify the actual query plans and
privacy/result regressions as well.

Opening a retained DHT database applies SQLx migration `20261116000000`.
A future dependency replacement must recognize that migration history/checksum,
or provide an explicitly reviewed, non-destructive migration reconciliation.
An unpatched binary may reject a database with an unknown migration. Do not
remove migration records, reset a conductor or discard identities to make a
replacement binary start. Preserve the existing migration files unchanged.

## Pinned 0.7.1 initialization/source-chain backport

`holochain` and `holochain_state` are copies of the published 0.7.1 crates, with
Lightning Rod Labs commit
[`ff9cf800c76aebe1c84b6ec40dab6c8bc0bc78bf`](https://github.com/lightningrodlabs/holochain/commit/ff9cf800c76aebe1c84b6ec40dab6c8bc0bc78bf)
backported from its newer development base. The data-layer portion is applied
on top of the existing `holochain_data` fixes above, not in place of them.
See [Holochain issue 6060](https://github.com/holochain/holochain/issues/6060).

The new native runtime changes are limited to six files:

- `holochain/src/conductor/cell.rs`: cache successful initialization on `Cell`,
  checking the flag again after taking the existing initialization mutex.
- `holochain_state/src/source_chain.rs` and `src/dht_store/reads.rs`: pass the
  action-type and sequence filters down to the data layer. Hash-bounded queries
  still load the chain required for traversal; final filtering remains in place.
- `holochain_data/src/dht.rs`, `src/dht/inner/action.rs`, and
  `src/dht/db_operations/action.rs`: apply those optional filters in SQL.

The effective [backport patch](holochain-init-backport.patch) is relative to the
published 0.7.1 crate sources, before the previously adopted data-layer fixes.
The 0.7.1 import spelling is adapted and two hunks targeting tests absent in
0.7.1 are omitted. Upstream source-chain prefilter regression tests are retained.
No existing migration or key-wrapping code is changed by this backport.

The three crates remain version **0.7.1**. Exact native dependency constraints and
both workspace lockfiles prevent silently dropping the backport during resolution.
The normalized conductor/state manifests also pin their patched state/data edges
at `=0.7.1`. Their original published manifests are retained as `Cargo.toml.orig`.
Registry bookkeeping and nested crate lockfiles are not vendored.

Upstream crate licensing is preserved: `holochain` declares CAL-1.0 (the included
`LICENSE` comes from upstream commit `c5f5028274c0406cb9d88d0fa48080135137fd75`);
`holochain_state` and `holochain_data` declare Apache-2.0. Source/checksum and
packaging provenance are recorded in [holochain-init-backport.json](holochain-init-backport.json).

Matched authenticated relationship reads on the historical diagnostic base fell
from 191.939 ms to 3.6525 ms complete-call median. The per-call outside-adapter
residual median fell from 187.2245 ms to 1.373 ms, a 99.27% reduction.
A complete `npm test` on that integrated historical worktree passed in
31m 24.145s, including 95 Sweettest checks with zero failures and seven standard
ignores. These are historical experimental results, not a claim that this
current-main PR head has run the identical full pipeline. See the
[evidence notes](holochain-init-evidence/README.md) for raw samples, run order,
timing conditions, source hashes, and validation boundaries.

### Removal and upgrades

Prefer an official reviewed Holochain release containing equivalent initialization
and source-chain query corrections. When upgrading, update the exact pins and
both native patch declarations together and rerun the authenticated diagnostic,
MAP integration suite, and native conductor/credential lifecycle tests.
Do not discard the separate DHT migration or credential-wrapping requirements
above when replacing these source copies. The backport adds no migration.
