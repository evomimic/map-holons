# Commit performance measurements

This procedure supports the end-to-end investigation in [Issue 816](https://github.com/evomimic/map-holons/issues/816). It measures existing behavior before choosing an optimization. Transaction, validation, and persistence semantics remain governed by the authoritative [Commit Validation](https://memetic-activation-platform.github.io/map-dev-docs/core/validation/commit-validation-design-spec/) and [Transactions](https://memetic-activation-platform.github.io/map-dev-docs/core/transactions/transactions-design-spec/) specifications.

## Run the baseline

From the repository root, inside `nix develop`:

    npm run sweet:commit-perf -- --output /private/tmp/commit-baseline

The output directory must not already exist. The command builds the production hApp once and runs five isolated test processes. Each process bootstraps the generated Core Schema in a fresh Sweettest conductor, then performs six sequential ordinary Commit commands. Use `--runs 1 --samples 3` for a short first-use/repeated experiment. Build and bootstrap time are excluded from the measured Commit interval.

Each sample stages one new `VisualizerUsage.HolonType` instance with these outbound members:

| Relationship | Target |
| --- | --- |
| `DescribedBy` | `VisualizerUsage.HolonType` |
| `UsesVisualizer` | `DefaultPropertyMapVisualizer.PropertyMapVisualizer` |
| `ForSubjectType` | `HolonType.TypeDescriptor` |
| `OwnedBy` | Bootstrapped local Space, supplied by staging |

This reproduces the four-member Commit shape from the original usage-selection capture. It does not execute the surrounding UI or `select_visualizer_usage` workflow, nor reproduce that capture's exact persisted corpus and relationship populations.

The harness requires `Complete`, one attempted/saved candidate, and no validation violations. Fresh reads verify each forward member and its inverse after the measured interval. Failed commits or assertions fail the run; incomplete captures are rejected by the summarizer.

## Cache and storage conditions

`first` means the first ordinary usage Commit after bootstrap in a fresh conductor/process. Bootstrap has already warmed conductor/WASM and host caches. It is not an OS-cache-cold or first-ever zome invocation measurement.

`repeated` means five subsequent new usage commits in the same runtime, each with a fresh transaction and guest request. Host/conductor caches can survive; guest membership caches do not survive requests. Inverse and ownership buckets grow by one usage per sample. The recorded `prior_usages` count identifies that changing population. Post-commit assertion reads also affect host caches, so compare matching sample positions as well as pooled repeated results. A same-runtime repeat does not imply identical storage or cache state.

Use additional fresh processes for repeatability. Do not clear caches or alter the generated schema between compared conditions unless explicitly defining a separate experiment.

## DescriptorPackage hypotheses and measurements

The initial hypotheses are that package preparation reaches more schema than subject validation needs, and that explicit cache preheating may be unnecessary compared with on-demand reads. Instrumentation distinguishes:

| Record | Meaning |
| --- | --- |
| `commit-assessment` | Exclusive sequential phases: prospective-view setup (`Construction`), readiness anchors, subject preparation/classification, Schema worksets, package discovery, prepared products, ownership preparation, package validation, and instance validation |
| `commit-construction` | Aggregate from assessment entry to package validation; contains the construction subphases |
| `commit-preparation-operation` | Aggregate elapsed microseconds and relationship backend work for eager `MembershipPrefetch`, subsequent `RelationshipTraversal`, and effective contract/constraint/binding computation; prefetch and traversal are grouped by preparation role |
| `commit-preparation-edge` | Logical reference requests and returned members by edge/role within discovery; these are not backend misses |
| `commit-preparation` | Distinct package roots, prepared content identities, identity/role visits, affected Schemas, subjects, and distinct prepared-product consumers |

Every visited content identity receives a contract, effective constraint product, and effective binding product. `contract_consumers` counts distinct governing packages requested during candidate assessment. Constraint and binding consumers count distinct prepared definitions requested through package accessors, including products that return errors or empty sets. Authored-surface consumers count requested source identities. These are direct consumer counts; they do not prove that indirectly traversed dependencies, canonical anchors, or unrequested products can safely be removed. They also do not count independent descriptor-kernel computation elsewhere in validation.

Eager prefetch spans include `SmartReference::prepare_relationships`; traversal spans include reads of the prepared relationships, discovery demand accounting, and queue expansion. Their backend counters can show whether prefetch fetched memberships subsequently served from cache. Zero backend reads after prefetch are not proof that prefetch is required. An on-demand comparison must preserve the same applicable checks and reports, and compare total work/latency across construction and validation, not only move reads out of construction.

Operation spans are sequential within each construction phase; they are nested in phase totals. Saved-content retention has its own operation span. Their remainders include identity selection, queue/bookkeeping, root assignment, instrumentation, and other uninstrumented work. Do not add operation totals to parent phase totals. `exact_node_reads` counts attempts through the storage `get_holon` entrypoint, not every possible Holochain `get_details` call. Timing probes use guest `sys_time`; unavailable clocks currently produce zero elapsed values, so verify nonzero parent spans before interpreting timings.

## Write side and boundary measurements

The existing `PERF-688 guest_commit` record reports node persistence, relationship persistence, inverse resolution, SmartLink bucket expansions, action creation, keyed ownership lookups, lineage materialization, and inverse deduplication. `commit-relationship-preparation` additionally separates relationship-descriptor resolution from endpoint identity/key resolution.

`commit-adapter` reports guest hydration, binding, dispatch, response projection, and session export. `commit-transport` reports the Sweettest conductor-call duration through response delivery. `commit-baseline` reports the whole host Commit command. The production WebSocket path additionally emits `commit-client-transport` with `MAP_PROFILE=1`, separating encoding, call, decoding, and total time.

The conductor round trip includes boundary work outside the guest timer. Its difference from guest execution is an unattributed residual, not a direct measurement of durable source-chain flush, transport, or validation individually. Those require conductor-level profiling if the residual is material. Microsecond action-creation times measure guest calls, not the complete durable-write cost.

## Captured artifacts and comparison

The runner stores raw per-process logs, build/corpus/source hashes, revisions, dirty status, logging configuration, toolchain/platform details, and a tracked working-tree patch. `measurements.json` preserves each sample and reports median, minimum, maximum, sample count, and standard deviation by condition and metric. Repeated samples within one process share state; isolated processes provide the independent runs.

To regenerate the summary from existing logs:

    python3 scripts/measure-commit-performance.py --summarize-only --output /private/tmp/commit-baseline

Compare the same schema hashes, logging filters, sample positions, and environment, recording artifact differences for deliberate code changes. Keep raw logs. Do not use brittle elapsed-time assertions as correctness gates. The runner does not implement modular commits or select a performance optimization.

`--variant on-demand` labels a separately built countermeasure and permits the intentional absence of eager-prefetch operation records. It does not switch runtime behavior. The implementation must already match the selected label; an on-demand capture containing eager-prefetch probes is rejected. When summarizing a shorter capture, supply its original `--samples` and `--variant` arguments.

## Eager-prefetch removal experiment — 2026-10-12

The first countermeasure removed the explicit `prepare_relationships` call from DescriptorPackage graph discovery. Ordinary bound-reference reads populated the request-local membership cache instead. The discovery edges, retained content, prepared products, and validation checks were preserved. The separate ownership preparation for affected Schema worksets was unchanged; the measured instance Commit has no affected-Schema workset.

The comparison uses sample positions 0–2 from four completed baseline processes and one countermeasure process with three samples. The paused partial fifth baseline process is excluded. The schema bundle hashes, repository revision, arm64 Mac environment, Rust/Cargo 1.94.0 toolchain, and logging filters match. Artifact hashes and working-tree patches record the intentional source differences. This is a screening experiment, not a statistically complete countermeasure baseline.

| Measurement | Batched prefetch | On-demand discovery reads |
| --- | ---: | ---: |
| First-use command, median | 17.63 s (4 samples) | 40.19 s (1 sample) |
| Repeated command, median | 16.69 s (8 samples) | 43.79 s (2 samples) |
| Repeated command range | 15.70–18.30 s | 42.68–44.90 s |
| Construction relationship backend calls, every sample | 380 | 1,317 |
| Construction logical link queries, every sample | 420 | 1,317 |
| First-use construction returned links | 1,917 | 584 |
| Construction exact node retrieval attempts, every sample | 183 | 183 |
| Prepared content identities, every sample | 127 | 127 |
| Identity/role visits, every sample | 131 | 131 |

All three countermeasure commits completed with one saved candidate, no validation violations, and successful fresh forward/inverse assertions. The 89 validation tests and guest/WASM compile check passed for the countermeasure, including malformed saved governors, prospective changes, and preparation/assessment tests.

Removing eager prefetch alone preserved the tested semantics but increased backend calls by 3.47 times and substantially increased latency. The named reads returned fewer links while losing the batching benefit of the existing explicit preparation path. Broad discovery continued to prepare the same 127 identities: this experiment did not implement demand-driven schema preparation or the proposed effective-view boundaries.

The countermeasure was rejected and the original graph-prefetch implementation restored. The next candidate should reduce the selected preparation closure or requested membership demand while preserving efficient batch retrieval and the existing correctness guarantees. These results do not establish which individual schema memberships can safely be omitted.

Local captured evidence:

- Baseline logs and metadata: `/private/tmp/map-commit-baseline-816-capture/` (complete runs 01–04).
- Countermeasure logs, metadata, and measurements: `/private/tmp/map-commit-on-demand-816/`.
- Baseline summary, comparison, and the rejected patch relative to the instrumented baseline: `/private/tmp/commit-prefetch-experiment/`.

## Preparation type inventory — 2026-10-12

A first-use and a repeated Commit reproduced 127 content identities, 131 identity/role visits, 380 construction relationship backend calls, and 183 exact node retrieval attempts. Discovery visit probes use direct `DescribedBy` keys already carried by references; they do not fetch content solely for type labels. Both samples produced identical counts, visits, backend calls, and node reads per type. Returned-link volume can grow with inverse-bucket population.

The table attributes work triggered during discovery visits. Its backend columns cover PackageDiscovery, not the earlier shared readiness phase. Readiness resolution accounts for the remaining 123 relationship backend calls and 60 exact node retrieval attempts; subject preparation accounts for one of each.

| Direct describing type | Content identities | Identity/role visits | Discovery backend calls | Discovery exact node reads |
| --- | ---: | ---: | ---: | ---: |
| `MetaDeclaredRelationshipType.MetaRelationshipType` | 27 | 27 | 54 | 27 |
| `MetaPropertyType.MetaTypeDescriptor` | 26 | 26 | 52 | 26 |
| `MetaHolonType.MetaTypeDescriptor` | 18 | 22 | 40 | 14 |
| `HolonValidationRule.HolonType` | 17 | 17 | 34 | 17 |
| `MetaValidationRule.MetaHolonType` | 8 | 8 | 16 | 8 |
| `MetaEnumValueType.MetaValueType` | 6 | 6 | 12 | 6 |
| `Schema.HolonType` | 5 | 5 | 10 | 5 |
| `CardinalityConstraint.ConstraintType` | 4 | 4 | 8 | 4 |
| `MetaValueType.MetaTypeDescriptor` | 4 | 4 | 8 | 4 |
| `MetaConstraintType.MetaHolonType` | 2 | 2 | 4 | 2 |
| `MetaIntegerValueType.MetaValueType` | 2 | 2 | 4 | 2 |
| `MetaStringValueType.MetaValueType` | 2 | 2 | 4 | 2 |
| `BooleanValidationRule.HolonType` | 1 | 1 | 2 | 1 |
| `EnumValueValidationRule.HolonType` | 1 | 1 | 2 | 1 |
| `IntegerValidationRule.HolonType` | 1 | 1 | 2 | 1 |
| `PropertyValidationRule.HolonType` | 1 | 1 | 2 | 1 |
| `StringValidationRule.HolonType` | 1 | 1 | 2 | 1 |
| `VisualizerUsage.HolonType (fixture-identified)` | 1 | 1 | 0 | 0 |
| **Total discovery** | **127** | **131** | **256** | **122** |

The one unlabeled visit is the sole staged subject in this fixture, a VisualizerUsage. Its saved governor reference did not carry a cached Key, so the probe deliberately avoided loading a label; the fixture identifies the type. It is not a missing descriptor. The other 126 prepared identities are supporting saved holons. The four extra role visits are all on holons described by `MetaHolonType.MetaTypeDescriptor`.

Evidence: `/private/tmp/map-commit-types-816/measurements.json` and `/private/tmp/map-commit-types-816/type-breakdown.json`. This inventory measures preparation and does not establish that every listed type is required or fully revalidated.

## Forced relationship retrieval comparison

Run inside `nix develop`:

```sh
npm run sweet:relationship-perf -- --output /private/tmp/map-relationship-retrieval --runs 3 --samples 6
```

This builds and audits the production artifacts and the isolated probe zome, then runs a dedicated ignored Sweettest target. Production retains its existing adaptive heuristic. The probe compiles the same SmartLink storage source without linking production externs and forces three strategies: separate exact-prefix calls, a single batched exact-prefix call, and broad source retrieval partitioned into the requested relationship collections. Forced broad retrieval omits the heuristic's count probe; its cost must be added when assessing an adaptive policy.

Each fresh conductor process authors six source workloads outside the timed interval: 0, 16, 128, 129, and 512 links distributed uniformly across 16 names, plus 512 links with 497 in one name and one in each of the other 15. Each source is queried for 1, 2, 4, 8, and 16 names, starting at name 0, 8, and 16. These windows include dense, sparse, partly empty, and entirely empty requested memberships. Repeated target identities carry distinct occurrence IDs so individual link occurrences remain testable.

Each cell performs one warm-up per strategy and rotates the three strategies' call order during measured rounds. These are warm-conductor storage measurements; runtime relationship membership caching is bypassed equally for all strategies. They do not measure cold distributed-network behavior or complete Commit latency. All strategies are checked against the seeded per-name counts and against each other's exact decoded SmartLink collections, including empty buckets. No holon target hydration is timed.

`run-NN.json` contains individual guest retrieval/decode timings and native round-trip timings, backend calls, logical queries, and returned-link volume. Native round-trip time additionally includes result serialization and conductor dispatch. `summary.json` groups each workload/strategy and reports median, range, and standard deviation across processes. `comparison.csv` puts the three guest-time medians side by side, reports the broad/batched ratio, and identifies the fastest observed strategy per cell. Metadata records artifact hashes, toolchain, revision, benchmark source fingerprints, and a working-tree patch; raw logs are preserved. There are 90 workload cells and 270 workload/strategy groups per process.

Use an empty output directory. `--skip-build` is available only when the current artifacts have already been rebuilt and audited. The benchmark makes correctness assertions but imposes no timing threshold; performance variability is reported rather than treated as a semantic failure.

### Initial forced-strategy results — 2026-10-13

The standard capture completed three fresh conductor processes, with six measured rounds per strategy per workload after warm-up: 4,860 timed reads across 90 workload cells, or 18 samples per workload/strategy group. Every retrieval passed seeded cardinality, exact decoded-collection equivalence, and backend-call/query/returned-volume assertions. An earlier one-process, three-round screening capture also passed all 90 cells.

Representative pooled guest retrieval/decode medians for uniform distributions, requesting names starting at 0:

| Source links | Requested names | Selected links | Separate prefixes | Batched prefixes | Broad retrieval |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 128 | 1 | 8 | 0.831 ms | 0.841 ms | 2.195 ms |
| 128 | 4 | 32 | 3.118 ms | 3.091 ms | 2.188 ms |
| 128 | 16 | 128 | 12.529 ms | 12.242 ms | 2.432 ms |
| 512 | 1 | 32 | 2.261 ms | 2.274 ms | 7.912 ms |
| 512 | 4 | 128 | 8.737 ms | 8.994 ms | 8.202 ms |
| 512 | 16 | 512 | 33.949 ms | 35.438 ms | 8.749 ms |

These measurements demonstrate a workload-dependent crossover. Broad retrieval also won some workloads above the current 128-link cutoff; the cutoff is not established as optimal by this experiment. Separate versus batched prefix timings were often close despite fewer host calls for batching, so host-call reduction alone does not predict elapsed time. These are local, warm-conductor measurements with fixed small tag payloads, and forced broad retrieval excludes count-probe overhead. They do not justify a universal threshold or an immediate production heuristic change.

Raw samples, per-group variability, and all workload comparisons are preserved at `/private/tmp/map-relationship-retrieval-816/`, including `comparison.csv`, `summary.json`, three logs, metadata, and the implementation patch. The screening capture is at `/private/tmp/map-relationship-retrieval-816-screen/`.

## Real Commit construction accounting — 2026-10-13

The first follow-up returns to the production VisualizerUsage Commit workload, rather than applying synthetic storage timings to a proposed smaller closure. Earlier temporary captures were no longer available, so fresh first-use and repeated commits were recorded. Existing phase/operation counters remain; a small guest-only scope timer partitions construction exclusively across HDK link calls (prefix, count probe, Single Scan, batched prefix), link decoding, runtime reference binding, membership-service bookkeeping, exact node retrieval/decoding, and the remaining construction work. Child time is excluded from its parent. The remainder includes cache/reference processing, semantic products, instrumentation, and any operations outside the explicitly timed read boundaries; it is not asserted to be entirely in memory.

The fresh control reproduced the broad 127-identity / 131-visit closure, 380 backend calls, 420 logical queries, and 183 exact node retrieval attempts. Construction was 14.595 s first-use and 13.655 s repeated; it did not reproduce the earlier 20.6 s literally. The dominant interval is nevertheless now directly located: the repeated Commit spent 12.738 s (93.3% of construction) inside HDK link-read calls. Those intervals include HDK marshaling/host-call overhead and Holochain processing/waiting; they do not isolate SQL or network time internally.

| Exclusive construction component | Repeated control |
| --- | ---: |
| Count probes used to choose retrieval strategy | 4.048 s |
| Single Scan calls | 3.941 s |
| Single Prefix Retrieval calls | 2.908 s |
| Batched Prefix Retrieval calls | 1.842 s |
| Exact node retrieval | 0.087 s |
| Node decoding | 0.098 s |
| Link decoding and reference binding combined | 0.009 s |
| Membership-service bookkeeping | 0.082 s |
| Remaining construction work | 0.642 s |
| **Construction total** | **13.655 s** |

The components reconcile with observed construction wall time to within 0–3 microseconds in the detailed control/trial captures. The capture parser checks this reconciliation and rejects inconsistent totals. These count probes are retrieval-strategy optimization work, not subject-cardinality constraint evaluation.

A temporary screening countermeasure skipped the count probe and selected Single Scan whenever preparation requested at least four missing names. Narrow/small-demand reads and freshness options were preserved; high-population sources that previously selected batched prefixes also switched to Single Scan. Exact requested buckets, filtering/decoding rules, discovery scope, and semantic preparation remained the same.

| Measurement | Control first | Trial first | Control repeated | Trial repeated |
| --- | ---: | ---: | ---: | ---: |
| Command time | 15.403 s | 8.431 s | 14.524 s | 8.728 s |
| Construction time | 14.595 s | 7.622 s | 13.655 s | 7.909 s |
| Relationship backend calls | 380 | 254 | 380 | 254 |
| Logical queries | 420 | 255 | 420 | 255 |
| Returned links | 1,917 | 2,766 | 1,918 | 2,768 |
| Exact node retrieval attempts | 183 | 183 | 183 | 183 |
| Prepared identities / visits | 127 / 131 | 127 / 131 | 127 / 131 | 127 / 131 |

Both trial commits completed with the expected single saved candidate, no violations, and successful persisted forward/inverse assertions. Repeated construction fell by 42.1%, and repeated command time by 39.9%. The reduction combines eliminated count probes with changes from batched prefixes to Single Scan; it is not a count-probe-only estimate. This is one process per variant with two commits, useful screening evidence rather than a representative production-policy baseline.

The production heuristic was restored exactly after the experiment. The remaining repeated-trial construction still spends approximately 7.015 s in link-read calls. The next work should therefore stay on actual host-side link-read costs and reduce costly calls or unnecessary preparation demand, rather than optimize millisecond-scale decoding or binding work. The original synthetic retrieval timings do not predict these real-call latencies.

Evidence:

- Coarse read accounting: `/private/tmp/map-commit-accounting-816/`.
- Detailed control: `/private/tmp/map-commit-accounting-shapes-816/`.
- Trial, exact screening patch, and comparison: `/private/tmp/map-commit-no-count-probe-816/`.

Each capture retains raw logs, measurements, toolchain/environment/artifact fingerprints, and the working-tree patch. The exclusive timers are inactive outside profiled Commit assessments, including storage-only retrieval probes.

## Populated-database index experiment — 2026-10-13

The explicit `--index-mode control|base-hash` capture retains an ordinary disposable SweetConductor after Core bootstrap. It inspects authored, DHT, and cache databases through their Holochain database handles. The indexed variant creates `Action_base_hash_perf_idx ON Action(base_hash)` only in these disposable databases, before measured Commits. Production schema and dependencies are not patched.

The test uses Holochain 0.6.3's actual `GetLinksQuery` SQL and parameters, records `EXPLAIN QUERY PLAN`, and measures five warm SQL executions after one warm-up. Each database's before/after observations use one transaction and assert identical sorted Action blobs. It samples the largest SmartLink source and a source with 4–16 link actions, using Single Scan, an exact-key Owns prefix, an Owns relationship prefix, and a Successor prefix. These direct SQL measurements exclude conductor dispatch, decoding, and network work.

Both populated databases held 11,472 Action rows and 34,413 DhtOp rows after bootstrap. The largest sampled source had 972 link actions, including 957 matching Owns links. The control plan starts both UNION branches with `DhtOp_validation_status_idx`, joins Action by its hash index, and applies the base/tag filters afterward. An exact-key Owns query returning one row and a Successor query returning zero rows both took approximately 14 ms **per SQL execution against one populated database**. An HDK link retrieval can query both populated stores, so these are not 14 ms per complete retrieval call. This explains why matching-link counts alone did not predict real Commit retrieval latency.

Creating the base-hash index alone did not change the selected query plan: SQLite still chose the status-first join and did not use the new index. Index construction took about 4.4 ms in authored and 6.1 ms in DHT. Direct SQL timings varied after creation despite the unchanged plan; those differences alone do not establish a source-index benefit. Planner statistics or query shape would need a separate experiment before claiming that the index addresses the observed global-store work.

Reproduction:

```sh
nix develop --command python3 scripts/measure-commit-performance.py --runs 1 --samples 3 --index-mode control --output /private/tmp/map-commit-index-control-816
nix develop --command python3 scripts/measure-commit-performance.py --runs 1 --samples 3 --index-mode base-hash --output /private/tmp/map-commit-index-base-816
```

Each directory preserves query plans and SQL timing samples in `run-01-index.json`, together with the ordinary Commit logs, parsed measurements, metadata, and implementation patch.

The complete Commit comparison used one process per variant and three commits per process (one first-use, two repeated). Both variants passed validation and persisted forward/inverse assertions.

| Metric | Control | Base-hash index only |
| --- | ---: | ---: |
| First command | 15.193 s | 15.733 s |
| Repeated command median | 15.393 s | 15.777 s |
| First construction | 14.373 s | 14.924 s |
| Repeated construction median | 14.597 s | 14.957 s |

This screening comparison provides no evidence of improvement from index creation alone. The indexed variant was slightly slower, and retained the same query plan. No production index migration is justified by this result.

## Bounded VisualizerUsage demand audit — 2026-10-13

This is a source/fixture audit cross-checked against the existing repeated Commit capture at `/private/tmp/map-commit-index-control-816/`; it adds no timed trial and makes no savings prediction. The sample creates one new staged VisualizerUsage with a present string Key and four authored relationships: DescribedBy, OwnedBy, UsesVisualizer, and ForSubjectType. Endpoint key resolution occurs before the measured Commit.

The governing lineage is `VisualizerUsage.HolonType → HolonType.TypeDescriptor → TypeDescriptor`. Its effective instance surface contains one property, Key, and ten relationships: DescribedBy, OwnedBy, Predecessor, UsesVisualizer, SelectedForSlot, ForSubjectType, PropertySalienceOverrides, RelationshipSalienceOverrides, ActionSalienceOverrides, and ActionGroupOverrides.

| Check / consumer | Descriptor-side demand | Subject-side demand | Scope |
| --- | --- | --- | --- |
| Governing structural/category checks | Governing Extends lineage; instance-kind fields; designated roots used for category/descriptor classification | Direct DescribedBy and subject Extends | Subject prerequisites |
| NoUndescribedProperties | Effective InstanceProperties; member TypeName; effective holon ValidationBindings | Present property names | Instance ValidationPackage + subject |
| UndeclaredRelationship | Effective InstanceRelationships; member TypeName | All staged authored relationship names/targets, already in memory | Instance ValidationPackage + subject |
| RequiredPropertyPresence | Key.PropertyType effective ValidationBindings; conditional requiredness semantics | Key presence | Instance ValidationPackage + subject |
| BaseValueKindMatchesString | Key's effective ValueType; value-type Extends lineage and kind fields; effective value ValidationBindings | Key value and BaseValue kind | Instance ValidationPackage + subject |
| Mandatory-constraint support checks | Effective Constraints on governing type, Key declaration, and string value type; all empty here | No additional subject reads here | Instance ValidationPackage |
| Governing contract validation | Declaration lineages/kinds; property ValueType and relationship SourceType/TargetType; duplicate/redeclaration/structural checks | Ht is the assessed definition | Separate descriptor assessment |
| Schema aggregate checks | Components/Rules/DependsOn/ownership only for scheduled affected schemas | No affected schemas in this sample | Separate schema assessment |

Source-traced bindings for this fixture are NoUndescribedProperties, DescribingCategoryCompatibility, DescriptorMetaTypeCorrespondence, RequiredPropertyPresence, and BaseValueKindMatchesString (each suffixed `.ValidationRule`). The two category/correspondence bindings dispatch through descriptor-route handlers but assess the candidate's structural compatibility; they must not be confused with validating the governing type Ht as a subject. Existing timing capture does not log per-handler keys, so this list is source-derived rather than a new runtime dispatch trace.

The current compatibility implementation independently resolves these canonical rule identities and their three families: HolonValidationRule.HolonType, PropertyValidationRule.HolonType, StringValidationRule.HolonType. Expected declaring-family identities are HolonType.TypeDescriptor, PropertyType.TypeDescriptor, and StringValueType.ValueType. These are identity-comparison consumers, not evidence that Owns-based rediscovery is necessary. The other 18 built-in rule identities and four other rule families have no binding in this fixture. The complete table is only inspected for unfamiliar-rule placement; this sample takes the recognized-key path.

Readiness anchor disposition:

- TypeDescriptor, HolonType.TypeDescriptor, and MetaTypeDescriptor.HolonType are consumed by existing candidate classification/category checks. MetaHolonType.MetaTypeDescriptor serves the root exception, which this sample does not enter.
- PropertyType.TypeDescriptor and StringValueType.ValueType also serve active binding compatibility. ValueType.TypeDescriptor and DeclaredRelationshipType.RelationshipType additionally serve governing-contract validation, a separate scope.
- Rule.HolonType, Schema.HolonType, and ConstraintType.HolonType are consumed for candidate classification (negative results here); they are not needed to discover the governing instance contract through its relationships.
- The four bounded constraint anchors, cardinality anchor, and unique-items anchor have no declaration to check for this ordinary-instance sample. The existing orchestration still initializes the declaration context.
- The universal descriptor contract is constructed eagerly, but its minimum-enforcement method is skipped because Key is present. Missing-Key and abstract-descriptor cases require explicit coverage before changing that behavior. Requiredness can legitimately consult the property's describing contract for defaults; a scope boundary must allow that explicit supporting demand.

Measured preparation has 127 content identities / 131 identity-role visits, but package consumers request one contract, three binding products, three constraint products, and zero schema authored-surface products. These counters describe consumption of package products; they do not account for all direct descriptor-kernel reads. Schema discovery alone visits five schemas (50 membership requests) although the affected-schema workset is empty. ComponentOf, RuleOf, and DependsOn have no consuming schema aggregate check in this sample and should not expand its instance ValidationPackage merely because ownership is present.

The current instance evaluator does not evaluate relationship cardinality, endpoint conformance, or collection policy. The declarations nevertheless contain cardinality constraints: ExactlyOne on DescribedBy, OwnedBy, UsesVisualizer, and ForSubjectType; ZeroOrOne on Predecessor; ZeroOrMore on the other five relationships. A behavior-preserving performance trial must distinguish this delivered behavior from the complete future relationship contract. For that complete contract, include applicable relationship constraints and endpoint-conformance specification, with subject membership reads scheduled by the checks; do not recursively validate target instances by default.

The next bounded implementation trial should construct products for explicit instance demand, keep governing-definition and affected-schema validation as separately requested work, and preserve malformed/contested-input diagnostics and staged replacement selection. It must not declare Ht previously valid merely because its content is cached. The current authoritative design only establishes assessment-local acceptance; reuse of prior validation needs its own evidence/invalidation contract. Thus separating the scopes can be prototyped without pretending that descriptor acceptance evidence already exists. No production validation behavior was changed in this audit.

## ReadinessContext retirement experiment — 2026-10-14

The opt-in `instance-assessment-experiment` feature on the `holons` coordinator forwards to the guest and shared validation crate. Only assessments containing VisualizerUsage instances with no authored Extends enter this path; definition/schema commits retain the existing path. The experiment does not construct ReadinessContext or call saved-key resolution. Existing descriptor kernel checks receive graph-reached category references, collected through Extends/DescribedBy from the governing type and its effective declarations/value types. Seven role references remain necessary to reuse those checks; they are not independently rediscovered through HolonSpace. This is a bounded exploration, not a completed general retirement of all category inputs.

Static dispatch already exists. Binding placement/routing metadata is now separated from resolved reference identities. The experiment takes rule references from effective ValidationBindings, uses their keys to select handlers and policy, and checks family/declaring-family lineage against encountered keys. It intentionally omits independent canonical-rule reference comparisons. Unknown handlers still reject. The normal path retains its canonical identity and complete placement checks. No versioned rule-key contract is introduced here.

DescriptorPackage construction remains unchanged. Governing-contract structural/member checks still run in this assessment: no prior descriptor acceptance is inferred from cached content. Ordinary subjects enforce required-property minima directly; abstract descriptor subjects remain outside the experimental path. Operational/contested-input handling and extension-family policy require more coverage before generalizing this prototype.

A focused saved-graph comparison asserts identical complete reports and coverage observations for valid input, a missing required property, wrong value kind, extra property, and an ungoverned assembled undeclared relationship. A saved-key lookup counter proves zero additional saved-key calls on the experimental path. A mixed instance/governing-definition update is explicitly confirmed to fall back to normal assessment. The validation unit suite passes with the experiment feature enabled (90 tests).

The live comparison uses three timed VisualizerUsage commits in each fresh conductor, followed by six semantic outcomes: valid instance; extra property rejection; corrected retry acceptance; wrong Key kind rejection; undeclared relationship rejection; and the existing missing-Key persistence error. Requiredness is covered by the focused fixture's required Title property. The live VisualizerUsage Key is optional in validation, but persistence requires it and reports `Committed holon has no key`; this mismatch was discovered in the control and is preserved, not repaired here. No claim is made that this operational failure appends no actions.

Earlier control attempts stopped on test expectations: first, the ordinary relationship mutation API rejected an undeclared edge before Commit; second, missing Key failed during persistence rather than producing Complete. Their timing samples remain in the screening directories, but the completed corrected control is the comparison baseline.

Reproduction uses separately built artifacts (the Python `--assessment` argument labels/verifies the observed path; it does not enable the Cargo feature):

```sh
# Build and pack standard artifacts, then capture control.
npm run build:happ
nix develop --command python3 scripts/measure-commit-performance.py --runs 1 --samples 3 --readiness-cases --assessment standard --output /private/tmp/map-readiness-control-final-816
# Build the opt-in guest, then pack it.
CARGO_TARGET_DIR=happ/target cargo build --manifest-path happ/Cargo.toml -p holons --release --target wasm32-unknown-unknown --features instance-assessment-experiment
nix develop --command hc app pack happ/workdir --recursive
nix develop --command python3 scripts/measure-commit-performance.py --runs 1 --samples 3 --readiness-cases --assessment instance-experiment --output /private/tmp/map-readiness-instance-experiment-816
```

When building from outside happ, explicitly select its target directory (`CARGO_TARGET_DIR=happ/target`); otherwise the packer may read an older loose WASM. Normal builds leave the feature disabled. The measurement parser requires a RelationshipDerivedInputs/InstanceExperiment guest observation for every experimental sample and rejects mismatched assessment labels. Artifact surface audit passes with the experiment enabled.

The completed control and experiment each used one fresh conductor with three timed commits (one first-use, two repeated). All six live outcome signatures match exactly, including rejection codes/rule keys, saved-count assertions, corrected retry, and the missing-Key operational error. This is screening evidence, not a multi-process production estimate.

| Repeated median / count | Standard control | Instance experiment |
| --- | ---: | ---: |
| Complete command | 13.989 s | 12.217 s |
| Construction | 13.270 s | 11.610 s |
| Input preparation (profile phase ReadinessResolution) | 2.560 s | 1.134 s |
| Input preparation link queries | 123 | 55 |
| Construction backend calls | 380 | 311 |
| Construction logical link queries | 420 | 347 |
| Exact node retrieval attempts | 183 | 126 |
| Package discovery | 10.330 s | 10.123 s |
| Package discovery backend calls / logical queries | 256 / 296 | 256 / 292 |
| Prepared identities / identity-role visits | 127 / 131 | 127 / 131 |
| Package contract / binding / constraint consumers | 1 / 3 / 3 | 1 / 3 / 3 |

Repeated command time fell 12.7%, construction 12.5%, and input preparation 55.7%. The profile retains the ReadinessResolution phase name for comparison, but the experimental interval is relationship-derived input preparation, not construction of ReadinessContext. Zero saved-key resolution is additionally asserted by the focused fixture counter. The 55 graph queries supply inputs to retained kernel/category and governing-contract checks; removing the context does not imply those reads disappear. Package discovery remains the larger cost with unchanged identity/role scope. Its small query-count difference reflects cache state left by the changed input preparation.

Evidence is in `/private/tmp/map-readiness-control-final-816/` and `/private/tmp/map-readiness-instance-experiment-816/`. Both contain logs, measurements, metadata and source patch; `run-01-cases.json` preserves semantic signatures. The experiment also retains `comparison.json` and its DNA/hApp/loose coordinator WASM under `artifacts/`. Standard feature-disabled guest artifacts are rebuilt and repacked afterward; the experiment remains opt-in in source. No production index or ValidatorPackage replacement was combined with this trial.

## Bounded ValidatorPackage construction — 2026-10-14

The `validator-package-experiment` Cargo feature introduces a separate ValidatorPackage constructor for VisualizerUsage instance assessments with no affected Schema workset and no authored subject Extends. Other scopes retain the broad DescriptorPackage constructor. The replacement supplies the existing assessment product/state adapter; it changes construction and dependency planning, not reference/cache behavior or handler implementations. Saved content is retained through existing cache leases and all inputs use prospective selection.

The demand starts at the governing type, not the subject. Its effective holon contract follows Extends, InstanceProperties, InstanceRelationships, Constraints, and ValidationBindings. Property declarations add ValueType and the describing contract needed to interpret requiredness defaults. Value types follow their effective constraints/bindings and lineage. Relationship declarations supply SourceType/TargetType identities, lineage, configured constraints and bindings. Rule references lead to their describing families and family lineages; configured constraints lead to their concrete type and lineage. Requiredness support prepares the describing contract's property names and inherited DefaultValue data; it does not validate those supporting definitions as subjects. Source/target declaration targets are retained as references without recursively preparing their instance contracts. The delivered evaluator still does not enforce C4 relationship cardinality/endpoint policies; this experiment does not add those checks.

No ComponentOf, RuleOf, DependsOn, Components, or Rules traversal is induced by package construction. Subject property values and authored relationship collections remain in the separate subject evaluation path. Governing-contract structural/member checks still run within the assessment; no saved acceptance evidence is assumed. Definition commits, mixed governing-definition updates, and affected-Schema aggregates keep their existing path.

A second broad walk existed in commitment scheduling. The bounded package now supplies its explicit commitment dependency plan to the existing scheduler, preventing post-construction ownership/schema rediscovery. The broad path continues to derive its own dependencies normally. The dependency plan is assessment-local scheduling data, not another persisted graph or raw cache.

Focused comparisons use separate saved snapshots for broad and bounded construction. Valid input, missing required property, wrong value kind, extra property, undeclared relationship, unsupported attached constraint, and malformed relationship declaration preserve complete normalized reports and coverage. The bounded assessment performs zero schema relationship backend reads after construction. An owning Schema with a self-dependency is read by the broad control and never read by the bounded path; its existence does not turn an ordinary instance into a Schema aggregate assessment. The validation suite passes all 91 tests with both experiment features enabled.

Captures use three timed commits followed by the same six live outcome controls as the readiness experiment. The Python assessment labels verify actual root-prefetch and relationship-derived-input guest events; they do not enable Cargo features. Each variant is built and packed separately. The package-only capture is `/private/tmp/map-validator-package-only-816/`; the combined capture is `/private/tmp/map-validator-package-combined-816/`. The package-only live test passed, but its initial parser expected an outer operation timer that was overwritten by nested operation events. The parser was corrected to verify the distinct ValidatorRoot prefetch role and reparsed the intact log. Future artifacts emit a sequential ValidatorPackageSelection event instead of a nested outer timer.

Build/measurement commands:

```sh
CARGO_TARGET_DIR=happ/target cargo build --manifest-path happ/Cargo.toml -p holons --release --target wasm32-unknown-unknown --features validator-package-experiment
nix develop --command hc app pack happ/workdir --recursive
nix develop --command python3 scripts/measure-commit-performance.py --runs 1 --samples 3 --readiness-cases --assessment validator-package --output /private/tmp/map-validator-package-only-816
CARGO_TARGET_DIR=happ/target cargo build --manifest-path happ/Cargo.toml -p holons --release --target wasm32-unknown-unknown --features validator-package-experiment,instance-assessment-experiment
nix develop --command hc app pack happ/workdir --recursive
nix develop --command python3 scripts/measure-commit-performance.py --runs 1 --samples 3 --readiness-cases --assessment combined --output /private/tmp/map-validator-package-combined-816
```

Completed screening results use a fresh feature-disabled control captured after both experimental variants, avoiding reliance on the earlier readiness-only control. Each variant has one fresh conductor and three timed commits (one first-use, two repeated); elapsed-time medians are screening observations rather than multi-process production estimates.

| Repeated median / count | Fresh standard | ValidatorPackage only | Both experiments |
| --- | ---: | ---: | ---: |
| Commit command | 13.854 s | 6.378 s | 4.326 s |
| Construction | 13.061 s | 5.611 s | 3.576 s |
| Input preparation | 2.806 s | 3.053 s | 1.320 s |
| Package discovery | 9.869 s | 2.504 s | 2.230 s |
| Package discovery backend calls / logical queries | 256 / 296 | 64 / 77 | 61 / 71 |
| Construction backend calls / logical queries | 380 / 420 | 188 / 201 | 116 / 126 |
| Exact node retrieval attempts | 183 | 105 | 51 |
| Prepared content identities / demand visits | 127 / 131 | 48 / 61 | 48 / 61 |
| Contract / binding / constraint consumers | 1 / 3 / 3 | 1 / 3 / 3 | 1 / 3 / 3 |
| Backend calls during package validation / instance validation | 0 / 0 | 0 / 0 | 0 / 0 |

ValidatorPackage alone lowered repeated command time 54.0%; combined with the readiness experiment, the reduction was 68.8%. The new demand visit key includes primary/supporting status as well as content identity and semantic role, so visit counts describe each constructor's work rather than identically defined role taxonomies. Content-identity counts and backend operation counts are directly comparable. No Schema demand/ownership expansion occurs in the bounded builder or its commitment plan.

Both experimental captures passed the same six live outcomes and their signatures match the completed standard controls exactly. The new fresh standard capture passed three timed commits and persisted forward/inverse assertions; the negative signatures are cross-checked against the preceding completed standard correctness capture at `/private/tmp/map-readiness-control-final-816/run-01-cases.json`. Focused seven-case report/coverage comparisons and cold-snapshot no-post-construction-read assertions pass. All 89 default validation unit tests, all 91 combined-feature validation tests, and all ten capture-parser tests pass; formatting, diff checks, WASM builds, and artifact audits pass. The full repository Sweettest suite was not run for this bounded experiment.

Evidence directories: `/private/tmp/map-validator-package-standard-816/`, `/private/tmp/map-validator-package-only-816/`, and `/private/tmp/map-validator-package-combined-816/`. The combined directory retains `comparison.json`; both experimental directories retain their packaged artifacts under `artifacts/`, and the combined capture also retains the loose coordinator WASM. Standard artifacts were rebuilt, repacked, audited, and verified to exclude both experiment markers. Source features remain opt-in. This establishes a useful bounded replacement, not general support for all instance types or a durable descriptor-acceptance policy.


## Retained-input warm assessment probe — 2026-10-14

The opt-in `warm-validator-probe` feature enables both bounded experiments and adds three report-only assessment passes after normal preparation and the original assessment, before persistence. It retains the package, relationship-derived kind inputs, prospective reader, saved-content leases, and transaction-bound references within the same Commit invocation. Each pass reruns governing-contract structural/member checks, rule binding preparation/dispatch, requiredness, property/value validation, and authored relationship checks. Findings and coverage are freshly accumulated and must exactly match the original pass; an operational error or mismatch aborts before persistence. Rejection outcomes are still installed from the original assessment, and no subject mutation occurs between passes.

This measures the cost of evaluating already-prepared inputs. It does not implement cross-Commit caching, skip descriptor checks on the basis of prior acceptance, or establish an invalidation policy. Eligibility selection, input discovery, package construction, and initial subject preparation are outside the warm span. Subject properties and memberships are also already warm: an estimate using this span is optimistic for a new subject with uncached data. Rebinding, checking that a package remains applicable to a prospective view, and transporting reusable semantic products between transactions remain unimplemented. Authoritative Commit design currently defers cross-assessment reuse; this probe is operational evidence for considering that work, not a replacement design rule.

The guest records aggregate `WarmValidatorAssessment / SameTransactionProbe` elapsed time, three invocations, relationship backend calls/logical queries/returned links, and exact node attempts. The capture parser requires those three invocations for the `warm-probe` label and rejects a probe artifact labeled as an ordinary combined experiment. Normal command timing includes all three extra passes, so it must not be presented as a cached Commit measurement.

```sh
CARGO_TARGET_DIR=happ/target cargo build --manifest-path happ/Cargo.toml -p holons --release --target wasm32-unknown-unknown --features warm-validator-probe
nix develop --command hc app pack happ/workdir --recursive
nix develop --command python3 scripts/measure-commit-performance.py --runs 1 --samples 3 --readiness-cases --assessment warm-probe --output /private/tmp/map-validator-warm-probe-816
```

The completed capture contains one fresh conductor, three timed commits, and six live outcome controls. Each timed commit has three warm passes (nine timed passes in total). The repeated figures below use two repeated commits; this is a bounded screening result, not a latency percentile or production guarantee.

| Repeated measurement | Median |
| --- | ---: |
| Warm governing-contract and subject assessment, per pass | 28.00 ms |
| Warm relationship backend calls / logical queries / node attempts | 0 / 0 / 0 |
| Actual command, including three extra probe passes | 4.702 s |
| Actual command minus added probe spans | 4.618 s |
| Construction | 3.806 s |
| Command time outside measured validation | 783.29 ms |
| Relationship persistence (Pass 2) | 756.50 ms |
| Inverse declaration resolution, within Pass 2 | 383.50 ms |
| SmartLink existing-link expansion, within Pass 2 | 265.50 ms |
| SmartLink action creation, within Pass 2 | 5.00 ms |

The simple modeled warm command is `command_us - validation_ms * 1000 + warm_span_us / invocations`: 811.29 ms repeated median, with 808.52–814.06 ms for the two repeated samples. Millisecond rounding in the existing total-validation metric adds less than one millisecond of uncertainty. This is explicitly a model, not an observed cross-Commit cached command. It retains all measured persistence and transport cost, but excludes reuse eligibility checking, cross-transaction reference binding, discovering changed definitions, and cold subject retrieval. The full command still costs seconds because it constructs the inputs once on every invocation. Retaining inputs within the invocation proves that evaluation itself is small; it does not prove that reuse across transactions is correct or equally cheap.

All six live signatures exactly match the completed standard correctness capture, including the existing operational missing-Key persistence failure. Each warm pass must match complete cold-pass findings and coverage; the 91-test validation suite also passes with the probe enabled, including valid and rejecting inputs and mixed-definition fallback. All eleven capture-parser tests, WASM builds, artifact audit, formatting, and diff checks pass. The measurement subprocess's final parser initially referred to `count` while the guest emitted `invocations`; this was corrected and the passed test's intact log was reparsed without rerunning or changing timing data. The full Sweettest suite was not run.

Evidence is retained in `/private/tmp/map-validator-warm-probe-816/`, including `measurements.json`, `warm-comparison.json`, live outcome signatures, raw log, working-tree patch, and experiment artifacts. The capture's patch predates the parser-field correction; current source uses `invocations`. Default artifacts were rebuilt and repacked after the measurement, then audited and checked to exclude all experiment markers. The feature remains opt-in.

The evidence supports exploring reusable immutable validator semantics with prospective-view applicability checks and transaction-local rebinding. It also isolates the next write-side targets: inverse declaration resolution and existing SmartLink retrieval. Those account for most remaining elapsed time; actual action creation is a small part. No cross-assessment cache, acceptance evidence policy, index migration, or write-side change is delivered by this probe.


## Inverse-resolution cache audit — 2026-10-14

Relationship persistence resolves metadata twice per nonempty collection: `resolve_inverse_relationship_name` selects the declaration, checks its declared/inverse type lineage, follows HasInverse, and reads the inverse TypeName; `save_smartlinks_for_collection` then selects the declaration again and reads forward/inverse TargetBinding. These are separate descriptor interpretation passes. The existing aggregate inverse-resolution timer describes the first pass only, not all policy preparation.

Scoped read observations group saved membership requests by full source HolonId, relationship name, and hit/missing/expired/fresh outcome. Saved-node observations cover `get_rc_holon`; direct already-cached accesses elsewhere are not counted as node hits. The guest records elapsed time, backend calls/logical queries, and exact node attempts separately for InverseName and TargetBindings. Scopes collect counters only while enabled, preserve lookup policies, retain no content, and close on errors. Staged graph reads bypass the saved cache and are distinguished by zero backend counts rather than being called saved-cache hits. No deduplication or policy memoization was implemented.

The first capture, `/private/tmp/map-inverse-cache-audit-816/`, uses three ordinary single-instance commits with both bounded experiment features enabled. Each makes nine distinct saved membership misses during InverseName: four HasInverse lookups and five Extends lookups. It fetches five saved nodes. Repeated InverseName time is 372.70 ms median; all subsequent TargetBindings passes use cached memberships, with zero link/backend/node reads and 22.88 ms median total. No repeated missing source/name pair or freshness/expiry bypass was observed within a Commit. Each invocation starts cold again under the stateless guest architecture.

For one repeated sample, DescribedBy causes three membership reads and two node reads, while ForSubjectType, OwnedBy, and UsesVisualizer each cause two membership reads and one node read. The inverse descriptor's type lineage is checked as well as its HasInverse membership, explaining why one first-use relationship does not imply one backend call. Across both passes the saved membership observations include 136 Extends hits, 24 InstanceRelationships hits, and four HasInverse hits, as well as the nine misses. The initial capture printed abbreviated source labels; grouping inside the probe always uses full identities, and the bulk follow-up emits canonical identity bytes in hex for unambiguous external analysis.

Core Schema bootstrap, captured during initialization of that same test, performs 4,706 InverseName and 4,706 TargetBindings passes, with zero link backend calls and zero exact node retrieval attempts. Its staged graph is already in memory. The measured spans still total 2.143 s and 0.941 s respectively. This supports investigating repeated descriptor interpretation in large staged commits, rather than alleging repeated backend retrieval.

The existing performance fixture additionally accepts a subject count, retains default single-subject behavior, and verifies saved forward/inverse occurrences for every subject. `--subjects` is recorded in capture metadata and selects `MAP_COMMIT_PERF_SUBJECTS`. Focused cache tests assert one first miss and two subsequent hits for populated and known-empty collections; all 472 Core unit tests pass. Full WASM builds validate the shared observation code. These diagnostic timings include instrumentation and are screening observations, not production latency estimates.

The ten-subject follow-up is `/private/tmp/map-inverse-cache-bulk-run-816/`, with two timed commits and persisted forward/inverse assertions for all twenty saved instances. Its 40 InverseName passes per Commit still make only nine membership backend calls and five exact node attempts. All 40 TargetBindings passes make zero backend reads. Repeated missing source/name pairs are absent, and no expiry/fresh bypass occurs. In the repeated sample, InverseName costs 896.76 ms and TargetBindings 230.01 ms; there are 1,355 Extends hits, 240 InstanceRelationships hits, and 76 HasInverse hits across both passes. Full-source identity grouping confirms the same misses are amortized across subjects within the invocation. Repeated full-command time is 7.009 s; it is not ten independent Commit calls.

| Workload | InverseName passes | Membership backend calls | Exact node attempts | TargetBindings backend calls |
| --- | ---: | ---: | ---: | ---: |
| One VisualizerUsage | 4 | 9 | 5 | 0 |
| Ten VisualizerUsages together | 40 | 9 | 5 | 0 |
| Core Schema bootstrap (957 candidates) | 4,706 | 0 | 0 | 0 |

This establishes that saved membership caching works in these workloads. Repeated resolution is not equivalent to repeated fetching. A possible next countermeasure is a Commit-local prepared write policy, keyed by the effective governing descriptor and relationship in the same prospective view, supplying inverse name and both target bindings together. It could eliminate the duplicate interpretation pass and share products across subjects without cross-invocation state. This is a proposed experiment, not an implemented cache or measured saving; malformed-policy/error behavior and prospective replacement selection would need preservation.

The bulk launch initially stopped before running a test because artifact preservation created its intended capture directory first. It was rerun against a fresh directory; the completed evidence is the `bulk-run` directory above. Both live targets pass. The analysis script and full scope-level JSON are preserved with the bulk capture; each capture also has `descriptor-cache-analysis.json`. Default feature-disabled artifacts were rebuilt and repacked after the experiment, checked to exclude all three experiment markers, and audited. Eleven capture-parser tests, 472 Core unit tests, formatting, and diff checks pass. Full repository Sweettests were not run for this bounded diagnostic change. No production retrieval or persistence policy was changed.
