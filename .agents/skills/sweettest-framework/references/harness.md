# Dance Test Harness — Implementation Orientation

Read this when adding a step or debugging token resolution, Commit expectations, or persisted
reads. For fixture authoring, start with [SKILL.md](../SKILL.md). Exact layouts and signatures
belong in the source; enduring design belongs in the
[DevDocs harness specification](../../../../../map-dev-docs/docs/core/testing/test-harness-design-spec.md).

## Where to look

Paths below are relative to `tests/sweetests/`.

| Concern | Source |
|---|---|
| Step vocabulary and authoring API | `src/harness/test_case/test_steps.rs`, `adders.rs` |
| Tokens and snapshot states | `src/harness/fixtures_support/test_reference.rs` |
| Logical identity, heads, staging provenance, node ownership | `src/harness/fixtures_support/fixture_holons.rs` |
| Runtime resolution and append-only recording | `src/harness/execution_support/execution_holons.rs`, `execution_reference.rs`, `execution_state.rs` |
| Saved-content equivalence | `src/harness/execution_support/execution_reference.rs`, `equivalence_resolver.rs` |
| Pure Commit and graph checks | `src/harness/execution_support/commit_disposition.rs`, `persisted_graph_assertions.rs` |
| Bootstrap and probe isolation | `src/harness/helpers/test_context.rs`, `smartlink_test_control.rs` |
| Driver and step dispatch | `tests/execution_steps/dance_test_runner.rs` |
| Persisted reads and graph execution | `tests/execution_steps/persisted_read_support.rs`, `persisted_graph_executor.rs` |
| Commit dispatch and result recording | `tests/execution_steps/commit_executor.rs` |

`rstest_dance_test_suites` in `tests/dance_tests.rs` registers suites. Individual fixtures belong
in a suite's `test_cases` vector; new step match arms belong in `dance_test_runner.rs`.
Expected-panic and probe-backed targets reuse the runner; `fixture_authoring_tests.rs` checks
fixture gates without executing it. Put pure assertion/classification logic in `src/harness`
so its tests run once under `--lib`, rather than in each integration target.

## Tokens, heads, and realizations

Fixture authoring declares intent using in-memory snapshots. Execution realizes that intent
through bound runtime references. Keep these phases separate:

- `TestReference` is an immutable, opaque pair of SourceSnapshot and ExpectedSnapshot. Both
  carry a transient fixture snapshot and a TestHolonState; their temporary ids identify snapshots,
  not persisted nodes. Deleted expectations still carry an id, but no meaningful content.
- `FixtureHolon` tracks a logical entity's current expected head and last-live snapshot. State
  comes from the head; logical identity is its registry key. Only FixtureHolons registers snapshots,
  selects or advances heads, and mints tokens.
- `StagingSource` records new-root versus saved-version provenance to **validate** declarations,
  never to infer update disposition. `SavedIdentity` distinguishes OwnNode from AliasOf(source).
- `ExecutionReference` pairs an expected snapshot with an ExecutionHandle, whose variants are
  LiveReference and Deleted. ExecutionHolons records these by **expected snapshot id**, append-only.
  Several snapshot ids may realize the same persisted identity.

Head following happens during authoring. Use `derive_next_source` for a new operation, and the
existing relationship-target helpers for targets. They select current heads without mutating
historical tokens; post-delete source derivation uses the last-live snapshot. Ordinary callers
can keep passing older tokens after Commit.

At execution, `state.resolve_execution_reference` selects the source or expected identity via
ResolveBy and finds its recorded handle. Use Source for ordinary inputs and Expected for
relationship targets, Commit declarations, or verification subjects. Executors must not consult
live FixtureHolons, resolve fixture snapshots as persisted holons, or duplicate head-selection
logic. Saved/SavedLookup inputs resolve to SmartReferences; an uncommitted staged target from
another transaction is invalid.

Expected relationship members can carry historical snapshot ids. Saved-content assertions use
the finalized FixtureHeadIndex to redirect them to recorded heads. Use the existing equivalence
resolver rather than inventing another correspondence rule.

## Commit dispositions and retries

Commit expectations are attempt-scoped and independent of observed behavior. The create-only
`add_commit_step` derives NewRoot only when staging unambiguously established a root; update
provenance requires `add_commit_step_with_dispositions`. Every live candidate must be declared
exactly once. Rejected attempts and command-error expectations take no declarations and advance
no fixture heads.

| Disposition | Guest state in | Node write | SavedHolons | Result identity / residual state |
|---|---|---|---|---|
| NewRoot | ForCreate | PublishRoot | yes | new / Committed(new) |
| NoAction | ForUpdate | none | no | none / ForUpdate |
| GraphOnly | ForUpdateGraphOnly | none | yes | source / Committed(source) |
| NewVersion | ForUpdateNewVersion | PublishVersion | yes | new / Committed(new) |

FixtureHolons validates provenance and coverage, prepares declarations in author order, and
mints one result token per declared candidate, **including NoAction**. The executor derives
actual dispositions independently from retained staged handles and SavedHolons membership,
then matches results by saved identity, never by key. Disposition mismatches precede saved-count
and snapshot failures. A property write can promote GraphOnly to NewVersion; a stale declaration
must fail rather than silently change.

Staging clears copied predecessor lineage. Expected persisted snapshots follow the declaration:
NewRoot has no inherited lineage; NewVersion has exactly the staging source as predecessor;
NoAction and GraphOnly retain the source version's existing predecessor. Clone candidate
snapshots before changing lineage; never mutate the saved source's snapshot.

NoAction's result token binds directly to its saved source without consuming a SavedHolons entry.
NoAction and GraphOnly heads alias existing nodes, so several Saved heads may realize one node.
`count_saved()` counts non-deleted OwnNode heads. Comparing aliases against the same saved node
is safe only because graph-only mutations are non-definitional by construction. A SavedLookup
source remains partial through Commit; it must never become a complete saved-content snapshot.

Operational errors are separate from disposition, command errors, and semantic findings. Compare
**new occurrences** against a pre-attempt baseline, including kind and multiplicity; omission
expects none. Include retained committed entries eligible for relationship retries, using
ExpectedRetryParticipant. They produce no new SavedHolons result and keep their existing saved
mapping, even when the retry has zero live candidates. Each retry gets fresh expectations.

Supported Incomplete covers successful Pass 1 followed by relationship-persistence failure.
An uncommitted Pass 1 failure is unsupported, not NoAction, and does not establish whether a node
write occurred. Use the existing rejection checks for semantic findings. For classification,
identity ambiguity, error deltas, and diagnostics, read `commit_disposition.rs` and `commit_executor.rs`.

## Adding a step

Follow a nearby adder/executor with the same lifecycle behavior; check current signatures in code.

**Adder:** derive the source through FixtureHolons, clone it, apply expected effects, and freeze the
new expectation. Register a new logical entity or advance the existing head as appropriate;
retain prior state on expected failure. Mint through FixtureHolons, append the step, and return a
chainable token when needed. Operation parameters and step-level outcomes belong in DanceTestStep,
not TestReference. Never mutate a previously minted snapshot or hand-construct tokens.

**Executor:** resolve through TestExecutionState, dispatch the command, validate the outcome,
and record each new realization with `ExecutionReference::from_token_execution` and `state.record`.
Record against the expected snapshot id exactly once; never overwrite. Assertion-only steps,
expected failures, and relationship-only retries do not create another realization of an existing
token. Commit uses its specialized workset and recording path described above.

Initialize fixture builders with TestCaseInit and finalize exactly once after adding steps.
Finalization exports transient fixture data and freezes head redirection for execution; runtime
setup rebinds/imports that data. Executors must not depend on live fixture-authoring state.

## Initialization and assertion freshness

The harness owns bootstrap in `src/harness/helpers/test_context.rs`. `init_test_runtime` creates
a conductor-backed runtime, provisions Core Schema and its space anchor, then opens the first
ordinary transaction. Suites share bootstrap/schema setup while giving scenarios their own
transaction and execution registry. Do not reproduce bootstrap in ordinary fixture executors.

`init_probe_test_runtime` is the isolated variant, returning a narrow SmartLinkTestControl.
Its backend must never join an ordinary suite. Keep injection between fixture steps and drive
Commit through the same executor and declaration model.

Choose the read surface deliberately:

- `open_assertion_context` reuses the active transaction while it is open; otherwise it opens an
  assertion transaction and imports fixture transients. `loaded_holons_with_context` uses this
  for existing descriptor checks.
- `loaded_holons_with_fresh_context` always opens a new observer without disturbing the active
  transaction. Pair persisted relationship reads with `RelationshipReadHint::RequireFresh`:
  a new transaction alone does not invalidate the shared saved relationship cache.

Saved-content comparison checks complete saved properties and definitional content, skipping
partial lookup stubs. Fresh persisted graph checks cover non-definitional changes, materialized
inverses, and exact lineage, including extra or duplicate edges. Tokens distinguish same-key
versions; Key subjects require unique enumeration, and Successor traversal requires a unique
path. Empty lineage expectations assert absence. Use the existing read helpers and assertion
functions rather than duplicating them.
