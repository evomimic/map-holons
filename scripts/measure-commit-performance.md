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
