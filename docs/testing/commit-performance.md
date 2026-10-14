# Commit performance measurements

This guide describes the reusable measurement tools delivered by
[Issue 816](https://github.com/evomimic/map-holons/issues/816). Findings and archived
results live in the authoritative
[performance investigation](https://memetic-activation-platform.github.io/map-dev-docs/core/performance/issue-816-findings/).
Runtime semantics remain governed by the Commit validation and transaction specifications.

## Standard Commit capture

Run from the repository root inside `nix develop`:

```sh
npm run sweet:commit-perf -- --runs 1 --samples 3 --readiness-cases --output /private/tmp/commit-baseline
```

The output directory must not exist. The wrapper builds and packs the coordinator
with the explicit `commit-performance-profiling` feature. Ordinary builds leave
that feature disabled; INFO logging alone does not enable detailed guest profiling.
The feature adds observation only, preserving the standard ReadinessContext,
DescriptorPackage construction, validation, dependency scheduling, and persistence.

The test bootstraps Core Schema, stages one VisualizerUsage with a Key and four
outbound members (DescribedBy, OwnedBy, UsesVisualizer, ForSubjectType), then times
ordinary Commit commands. Bootstrap, staging, and endpoint lookup are excluded.
Fresh persisted forward/inverse assertions run afterward. `--subjects 10` commits
ten instances together. `--readiness-cases` additionally checks valid input,
extra-property rejection, correction/retry, wrong Key kind, undeclared relationship,
and the existing missing-Key persistence error.

`first` means the first instance Commit after bootstrap in a fresh conductor.
`repeated` samples share that conductor but use new transactions and guest calls.
Host/conductor caches can survive; guest membership caches cannot. Inverse buckets
grow, and assertion reads can warm host caches. These are not identical-state or
OS-cache-cold comparisons. Default capture size is five processes with six commands each.

The capture contains individual measurements, condition summaries, raw logs,
source/schema/artifact hashes, toolchain and environment metadata, and a working-tree
patch. Incomplete samples or mismatched artifact markers are rejected. No elapsed-time
threshold is asserted. To reparse a three-sample standard capture:

```sh
python3 scripts/measure-commit-performance.py --summarize-only --samples 3 --output /private/tmp/commit-baseline
```

The parser retains historical assessment labels (`instance-experiment`,
`validator-package`, `combined`, `warm-probe`) for archived experiment logs.
Those experimental implementations are preserved on the 816 checkpoint branch;
they are not part of this measurement foundation. Labels never build/select an implementation.

## Observations and timer boundaries

| Marker | Meaning |
| --- | --- |
| `commit-baseline` | Complete host Commit command |
| `commit-transport`, `commit-client-transport` | Conductor round trip and production client encode/call/decode timings |
| `commit-adapter` | Guest binding, dispatch, projection, and session export |
| `commit-assessment` | Sequential validation phases |
| `commit-construction` | Assessment entry through input construction, before package validation |
| `commit-preparation`, `commit-preparation-type`, `commit-preparation-edge` | Content/role visits, requested memberships, and distinct prepared-product consumption |
| `commit-preparation-operation` | Content retention, membership preparation/traversal, and effective semantic products |
| `commit-read-cost`, `commit-cost-accounting` | Exclusive construction costs; child spans excluded from parent costs |
| `commit-descriptor-resolution`, `commit-descriptor-cache` | Inverse-name/target-binding spans and scoped saved-cache hits/misses by source/name |
| `PERF-688 guest_commit` | Node/relationship persistence and nested write-side costs |

Parent and child timers overlap: do not add phase/operation totals indiscriminately.
Exclusive read costs reconcile to construction time; the parser verifies that reconciliation.
Counter distinctions matter: a batched backend call may issue several logical queries;
reference requests are not cache misses; returned links are not distinct content identities.
Node attempts count the storage `get_holon` entrypoint, not every possible Holochain read.
Saved-node cache observations cover `get_rc_holon`; direct already-cached access elsewhere
is not counted as a node hit. Staged graph access bypasses saved membership caching.

Clock probes use guest `sys_time`. Check that parent durations are nonzero before
interpreting data. HDK read spans include marshaling, database work, and waiting.
Guest action-call timings do not measure complete durable-write latency. Round-trip
residuals are unattributed unless conductor-level profiling supplies their components.

## Forced relationship retrieval comparison

```sh
npm run sweet:relationship-perf -- --runs 3 --samples 6 --output /private/tmp/relationship-retrieval
```

The isolated loose probe zome compiles the same SmartLink storage source without
importing production externs. The artifact audit excludes it from production bundles.
It forces Single Prefix Retrieval, Batched Prefix Retrieval, and Single Scan on
identical seeded workloads, bypassing membership caching and adaptive count probes.

Each process tests 90 cells: 0, 16, 128, 129, and 512 uniformly distributed source
links plus a skewed 512-link source; 1/2/4/8/16 requested names; and three name windows
including empty sets. Strategy order rotates after warm-up. Exact decoded results
must match. Timings exclude holon hydration and are not complete Commit measurements.
Individual JSON samples, summary variability, comparison CSV, metadata, and logs are retained.
Archived labels `separate_prefixes`, `batched_prefixes`, and `consolidated` correspond
to the three names above. No universal production threshold is assumed or changed.

## Disposable database-plan experiments

```sh
npm run sweet:commit-perf -- --runs 1 --samples 3 --index-mode control --output /private/tmp/index-control
npm run sweet:commit-perf -- --runs 1 --samples 3 --index-mode base-hash --output /private/tmp/index-trial
```

The explicit test mode inspects the disposable conductor's authored and DHT databases,
records Holochain's GetLinksQuery SQL and EXPLAIN plans, and optionally adds an
Action.base_hash index there. Before/after direct SQL results must match exactly.
`run-01-index.json` preserves SQL samples and plans. No production database or
Holochain migration is modified. Compare matched corpus/artifact hashes and
sample positions; changing a plan, strategy, or store population changes the experiment.

## Restore standard artifacts

After measurement, rebuild and audit ordinary artifacts before running the app:

```sh
npm run build:happ
npm run build:probes -w map-happ
npm run check:happ-artifacts
```

The archived evidence in DevDocs records historical experiments, rejected approaches,
limitations, and the two recommended enhancements. This guide remains the operational
entry point; new conclusions and measured results belong in that performance section.
