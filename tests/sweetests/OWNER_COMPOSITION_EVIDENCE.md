# Owner-directed composition verification

This is an implementation/test record, subordinate to the published
[DAHN design specification](https://github.com/memetic-activation-platform/map-dev-docs/blob/cf1c7cc0974e6a51a38a52d8a2801abae415f8ad/docs/core/hx/dahn-design-spec.md#1454-runtime-index-and-preparation-validity)
and [implementation plan](https://github.com/memetic-activation-platform/map-dev-docs/blob/cf1c7cc0974e6a51a38a52d8a2801abae415f8ad/docs/core/hx/dahn-implementation-plan.md).
It does not establish additional architecture or change the experience-edit deferral.

## Reproduce

Use the repository's supported native/WASM toolchain:

```sh
npm run build:happ
MAP_SWEET_STARTUP_TIMEOUT_SECS=60 WASM_LOG=warn RUST_LOG=warn cargo test --manifest-path tests/sweetests/Cargo.toml --test dance_tests owner_directed_composition_acceptance -- --nocapture
```

The standalone acceptance test runs in the normal suite and uses the production coordinator and a disposable conductor. It
constitutes a PersonAgent once, imports the collection package and test vocabulary,
and uses ordinary Commit. No retained application store is reset or migrated.

## Evidence boundary

`owner_directed_composition.rs` checks the client Commands → canonical guest →
consolidated result boundary. The guest performs child selection through shared
calls. The test does not substitute a host selector or per-child Dance dispatch.

The test exercises:

- First personal use with no Usage records; initial Inspector leaves collection Slots inactive.
- An explicit collection Demand and an alternate owner's declared initial collection source.
- Eighteen declared Date fields, all unset initially, including fields outside a four-row viewport.
- A cross-cutting Date preference without Usage across two owners; optional shared configuration;
  one local override; an incompatible range Slot; and populated/unset equality.
- The first saved preference replacing an observed empty personal population.
- Explicit old Slot-preference conversion and repeated conversion preserving its scope/identity.
- A new host Runtime against the same committed conductor state, without bootstrap or constitution.
- Actual cold/warm transport calls, payload size and elapsed time, separately from selector work.

Negative selector/conversion cases live in `dahn_selection/tests/property_selection.rs`.
Persistence, rejection, Incomplete remediation/resubmission and publication ordering
remain covered by `map_commands_runtime` tests. UI tests cover direct label/value
realization, descriptor-backed Table demand, callable admission and independent occurrences.

`CompositionWork` counts shared selector calls, within-round binding reuse, shared
metadata hits, personal-population requests and discovery-population requests.
These are semantic-operation counts, **not SQL/DHT query counts**. A prescribed
answer does not enumerate unused alternatives. The transport recorder separately
counts actual host/guest envelopes; its `canonical` count identifies composition
invocations. It reports additional cold initialization reads explicitly.

## Observed run — 2026-11-19

Production-coordinator acceptance passed in 160.11 seconds including disposable
conductor startup, schema bootstrap, package imports, ordinary commits and recovery checks.
The following intervals measure composition resolution, not that complete setup:

| Workload | Elapsed | Shared selections | Binding reuses | Personal population requests | Candidate population requests |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial Holon Inspector, 12 child bindings | 735.7 ms | 6 | 6 | 1 | 0 |
| PropertyMap with 18 Date fields plus inherited Key | 581.5 ms | 2 | 17 | 1 | 0 |
| Fresh host Runtime, same persisted Space | 829.5 ms | 6 | 6 | 1 | 0 |
| Repeated request in that host context | 853.1 ms | 6 | 6 | 1 | 0 |

The fresh-host sample had eight transport envelopes: one canonical composition
invocation, two descriptor-key lookups and five identity reads. The repeated
sample had three: one canonical invocation and two descriptor-key lookups.
Neither sample issued a transport call per child. The consolidated scalar result
was 19,271 bytes. The complete canonical response carrier measured 36,913 bytes
cold and 73,321 bytes warm; these measurements do not assert bounded carrier
growth within a retained transaction. Further protocol-envelope cleanup and the
two remaining key lookups are not hidden by the single canonical-invocation count.

Validation also passed 794 shared-core tests, 105 host runtime tests, 39 selector/
conversion tests, 3 wire tests, 401 SDK tests and 515 UI tests (one unrelated UI
skip), plus targeted schema tests, web/SDK typechecks, host workspace checking and
the production hApp/WASM build. Following the bootstrap and presentation test
corrections, targeted verification passed 19 schema tests, 59 harness unit tests,
9 fixture-authoring tests, the canonical loader integration (193 seconds), and
the Book presentation integration (163 seconds). The user subsequently confirmed
that the complete `npm test` suite passed on 2026-11-20. Packaged application
launch has not been reverified for this checkpoint.

## Reuse and remaining reads

A composition result is valid for its authoritative resolution round. It carries
exact references, observed preference/default/discovery populations (including
empty sets), selected contracts, implementation metadata and effective configuration.
It does not authorize indefinite winner reuse across rounds. A subsequent
resolution observes the current sovereign association and personal population;
a first preference save cannot reuse an earlier absent-population answer.
Already mounted occurrences retain independent state. Verified executable modules
are cached by exact selected version, separately from semantic selection.

The host still reads actual subject contents, activated collection populations,
and uncached executable bytes. It verifies artifacts and admits required/offered
callables. Root selection, action membership/interaction metadata and genuinely
unprepared information can still require reads. There is no claim that all Canvas
startup work or all existing presentation APIs have disappeared.

The measurements exclude executable materialization and browser paint; they are
not a packaged Canvas startup benchmark. A fresh host Runtime verifies persisted
personal recovery but does not simulate stopping/restarting the conductor process.
Full application restart, schema upgrade of the user's retained installation, and
end-to-end packaged performance remain separate live-verification work. The
implementation does not silently replay imports into that retained store.
