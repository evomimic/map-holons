# Native dependency patches

These patches apply only to the `host/Cargo.toml` workspace. They do not change
hApp/WASM dependencies or the root IDE workspace.

## `holochain_data` 0.7.1 — gossip sector range queries

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
- Regression tests for query plans, inclusive/wrapping/singleton arcs, half-open
  time bounds, warrants and private/unvalidated-op filtering.

The result set, privacy filters, gossip participation, keys, identities, operation
validation and Commit semantics are unchanged. Other queries are unmodified.
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
