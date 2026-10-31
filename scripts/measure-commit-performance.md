# Commit performance captures

The harness measures ordinary guest Commits with aggregate instrumentation. See the
authoritative [Commit validation design](../../map-dev-docs/docs/core/validation/commit-validation-design-spec.md)
for assessment semantics.

## Capture

Run from the map-holons root inside Nix. Each output directory must be new; failed
captures retain their logs for inspection.

```sh
nix develop
npm run sweet:commit-perf -- --runs 5 --samples 6 --mode create --readiness-cases --output /tmp/commit-perf/visualizer-usage
```

Each capture writes `metadata.json`, `working-tree.patch`, raw `run-*.log` files,
and `measurements.json`. Metadata records resolved runtime versions, package
substrate, anchor evidence policy, artifact and source hashes, revision, working
tree status and run dimensions. Keep the runtime, substrate and fixtures constant
when comparing implementation changes.

The `--assessment` option labels the separately built guest artifact; it does not
select an implementation. The summarizer checks the measured probes against that
label. The standard assessment uses DescriptorPackage.

For either instance creation mode, `--readiness-cases` also writes per-run semantic
controls in `run-*-cases.json`: valid, extra property, wrong key kind, undeclared
relationship, missing key, and the corrected extra-property retry. These controls
run after the timed samples.

## Workloads and boundaries

| Mode | Timed Commit |
| --- | --- |
| `create` | VisualizerUsage creation, with four outbound members |
| `visualizer-slot` | VisualizerSlot creation, a string description and three outbound members |
| `new-version` | A property-only successor of a saved, non-root PropertySlot version |
| `descriptor` | One concrete HolonType descriptor, extending the saved HolonType root, described by the saved MetaHolonType and owned by the saved Core Schema |

The descriptor workload affects one saved Schema. It creates no Schema, rule,
property or relationship definitions. Every sample asserts a complete Commit,
zero violations and the expected saved count. Creation workloads check persisted
forward and inverse links after the timed command.

Each run starts a new test process and isolated conductor. Sample zero is `first`;
the remaining samples are `repeated`, each in a new transaction. New-version setup
publishes an initial successor outside the sample boundaries. Repeated creation
samples grow shared inverse buckets, and descriptor samples grow Core Schema
membership. These are repeated executions, not identical storage snapshots or
production latency guarantees.

Measurements retain total command and construction time, logical link queries,
backend calls, exact node reads, and assessment phase and preparation operation
breakdowns. Aggregate query counters do not measure distinct keys or keyed lookup
invocations.

## Bootstrap measurements

`npm run sweet:bootstrap-perf` measures bootstrap in a fresh runtime. It rebuilds
the ordinary guest without the Commit profiling feature and reports bootstrap
totals in its logs.

Profiled Commit capture logs also retain bootstrap output before the first sample
marker. The Commit summarizer excludes this setup. Keep standalone bootstrap
results separate from profiled setup measurements, since their instrumentation
differs. Commit capture metadata does not describe standalone bootstrap runs.

## Re-summarizing

```sh
python3 scripts/measure-commit-performance.py --summarize-only --output /tmp/commit-perf/visualizer-usage
```

Re-summarization uses the capture's saved mode, sample count, assessment, variant
and `anchor_evidence_policy`. The `per-new-version` policy expects one anchor
evidence invocation per new-version sample and zero in creation modes. The
`retired` policy expects zero in every mode. Select the policy matching the guest
artifact with `--anchor-evidence-policy` when capturing. New captures default to
`retired`; use `per-new-version` explicitly when capturing an older guest artifact.

Older metadata without a policy uses `per-new-version`. Preserve archived metadata
so its measurements remain interpretable after the implementation changes.

## Issue #822 instrumentation (Phase 1)

New captures use `--instrumentation 822-v2 --preparation-substrate broad-readiness`.
The profiled guest emits a matching marker with `read_detail=disabled`. Rebuild with
`npm run build:commit-perf -w map-happ` before capturing. Version 822-v2 replaces the
unmeasured v1 draft's per-read logging and per-counter records; the parser rejects v1
markers rather than mixing formats. Historical pre-822 captures remain accessible through
their recorded metadata or `--instrumentation historical`.

Commit the instrumentation before the Phase 2 baseline capture. Metadata `revision` records
the instrumentation checkout revision; `working-tree.patch`, status and file hashes record
local differences. There is no separate duplicate instrumentation-revision field. Supply
`--guest-artifact-revision` with the actual guest build revision. `--semantic-revision`
defaults to `3d053ba8becbac13496d7531abea8ce9a5303c53`; set it for final measurements.
The local DevDocs revision and runtime versions are retained. Phase 1 supplies instrumentation,
not baseline performance results or an instrumentation-only commit revision.

Additional metadata options describe `--registry-population` (requested setup population),
`--guest-cache-state`, `--session-product-reuse`, and `--governor-presence` (established by
workload setup). Describe actual ingress and setup warming; a fresh transaction alone does
not establish cold cache or absence of Session products. Unknown conditions stay unavailable.
The reserved `declared-demand` substrate permits absent broad readiness observations only
with a matching final guest marker; it does not select a runtime implementation.

Operation spans are **exclusive siblings**, contained by phase spans. The scope guard used
by both `observe_operation` and block observations closes on normal completion and operational
error. Interrupted records are reserved for unmatched aborts, not ordinary `Result::Err` paths.
The helper takes an explicit role: definition product spans use `PreparedDefinition`, aggregate
work uses `Assessment`, and existing membership/content roles are preserved. Do not sum phase
and operation totals. Readiness discovery and dependency grouping remain separately timed.
Readiness edge records are aggregated by edge and emitted after those spans close.

Snapshot lookup/bypass, saved restoration, fresh computation, name indexes, and contribution
projection replace the composite EffectiveContract and empty constraint/binding spans.
Structural diagnosis, kind classification, Schema workset, binding roles, universal contracts,
graph-only classification and check categories have separate spans. Construction includes
preparation outside DescriptorPackages.

`commit-derivation` emits **one line per phase**, with all counters as fields, including zeros.
Resolution requests count before lineage discovery, even on failure before building. Builder
and restoration attempts/successes are distinct. `from_valid_lineage` builds without requesting
a new resolution. `name_index_builds` counts actual lazy initialization, including failures.
`requested_schema_products` is currently an alias of `distinct_content`: the broad baseline
requests a complete schema for every reached definition. Successful construction is counted
separately and does not establish semantic validity.

`commit-logical-reads` emits one aggregate line per phase: total requests, membership requests,
content requests, hits, misses, and distinct memberships. Misses include missing, expired and
forced-fresh requests. Requests equal both membership+content and hits+misses. Distinct counts
are phase-local; do not sum them to claim a Commit-wide distinct count. Logical demand,
traversed edge members, backend calls, returned links and exact node reads remain separate.
Both probes close at phase transitions and on success/rejection/error before Pass 2 begins.

Detailed exact source/name/outcome evidence is **opt-in inventory instrumentation**, for Phase 3
read-removal investigations only. Build the coordinator with `holons/commit-read-inventory`
(which enables profiling), for example:

```sh
CARGO_TARGET_DIR=target cargo build --manifest-path happ/Cargo.toml -p holons_integrity -p holons --release --target wasm32-unknown-unknown --features holons/commit-read-inventory
hc app pack happ/workdir --recursive
```

Keep those raw logs as inventory evidence. Never use that artifact for timed captures: the
parser rejects `read_detail=enabled`. Detail lines are never converted to metrics or summary
keys. Rebuild the ordinary profiling artifact before timing. Aggregate records have bounded
line counts independent of the number of recorded read keys. Profiling still has work-counting
and aggregation overhead; comparisons require the same instrumentation and conditions.

`commit-registry` records received snapshot state, its actual population counted **once per
Commit**, and assessment-wide bypass. Counting the transient root's PreparedDescriptors
membership does not hydrate descriptors or query a backend, and still runs when the snapshot
is bypassed. A metadata read failure is unavailable, never a profiling-induced operational error.
Governor presence comes from workload setup, with `registry_hits` as execution evidence.
`snapshot_lookups` counts only saved targets with a registry root; ineligible targets and absent
registries do not count. `registry_entries_examined` measures scan work independently of received
population. The former population-times-lookups counters have been removed.

Sweettest Commit workloads use `ClientHolonService::commit_internal` through the mock conductor,
so **host snapshot construction is measurable there**. The script enables `holons_client=info`
alongside the guest/harness logs. New captures record `host_snapshot_policy=required`;
the parser fails visibly if construction time or counts are missing. Archived/other paths may
retain an optional policy, where unavailable remains distinct from measured zero. With MAP_PROFILE enabled, `commit-host-snapshot` reports
construction time and counts taken from the returned transient root, including zero for an
absent snapshot. Snapshot counts no longer use the derivation probe. The timer stops before
counting/logging; metadata read failures do not change Commit behavior.

Sweettests bypass `HolochainConductorClient`, so their `commit-host-shipping` observation is
unavailable. The production caller emits complete-envelope encoding time and encoded size;
these include non-registry content. Snapshot-only serialization is unavailable without another
encoding pass, which instrumentation does not perform. Conductor round-trip time includes guest
execution and must not be added to it. Parsed samples distinguish unavailable host observations
from measured zero. Run the workload smoke tests and matched baseline manifest in Phase 2.
