---
name: sweettest-framework
description: >
  Author, extend, or understand MAP Holons integration tests in tests/sweetests/.
  Use before editing fixtures, DanceTestStep adders or executors, harness code,
  or direct conductor tests; first select the client or conductor evidence boundary.
---

# Sweettest Framework — Authoring Guide

Current API signatures live in the harness source; enduring MAP design lives in `map-dev-docs`.

## Choose the Test Boundary

- **Client-visible semantics:** use dance fixtures for staging, token chaining, Commit lifecycle,
  relationship graphs, and saved-content equality. Follow the authoring guidance below.
- **Integrity execution or substrate-record claims:** use direct conductor tests under the
  [Conductor Test Specification](https://github.com/memetic-activation-platform/map-dev-docs/blob/main/docs/core/testing/conductor-test-spec.md).
  Do not add a `DanceTestStep` just to expose substrate evidence. Each conductor test owns its
  backend; do not apply dance-suite setup sharing to these tests. Probe-enabled conductors
  must remain separate from production-only assertions.
- **Pure classification/reporting:** use unit tests. A standalone async test is not automatically
  a conductor-boundary test; choose by the evidence the assertion requires.

## Before Authoring a Dance Case

- **Identify the coverage gap.** Name the unique behavior and its test layer. Keep pure
  classifier/reporting combinations in units; retain representative real-boundary integration
  checks. Identify the required coverage owner before omitting duplicate assertions.
- **Justify each step.** Compose proven adders. Do not append verification as reassurance when
  Commit and existing harness checks already establish the claim. Before extending a shared
  step, inspect its callers and preserve their contracts; add missing shared checks narrowly.
- **Author expectations through the fixture.** Its TransientHolons and TestReferences supply
  parameters and expected results. Never reconstruct expected mutations from persisted actuals.
  SavedLookup stubs are partial knowledge; an omitted membership is not evidence of emptiness.
- **Bound the reads to the claim.** For small changes, use the declared subjects and affected
  named collections. Call out saved-root enumeration, schema-wide traversal and repeated
  descriptor discovery. Database-wide assertions need a database-wide requirement. Preserve
  fresh-observer checks, exact identities, multiplicity and meaningful absence/error distinctions.
- **Share compatible dance setup.** Prefer existing suites over another bootstrap. Preserve
  separate transactions and fixture registries, and fresh Sessions, caches or measured transports
  where those boundaries matter. Order initially-missing checks before steps that populate the
  membership. Sharing a conductor does not establish fresh host state; keep probes separate.

Choose the existing verification contract; these checks are complementary:

| Claim | Preferred mechanism | Limit |
|---|---|---|
| Saved properties and definitional content | `add_match_saved_content_step()` | Skips partial lookup stubs; does not prove every graph/lineage guarantee |
| Declared persisted memberships, inverse occurrences, multiplicity and exact lineage | Set `test_case.verify_committed_workset = true` during authoring | Verifies on Complete; retains original expectations through Incomplete retries |
| Rejected keyed creates remain absent from saved lookup; named existing collections remain unchanged | `add_commit_step_with_rejection_checks` | Unique nonempty-key new roots only; no updates or relationship retries after Incomplete; does not prove absence of inaccessible substrate records |
| Explicit graph requirement outside the shared guarantees | Standalone persisted-graph assertion | Do not repeat shared Commit checks; `VerifyPersistedGraph` currently enumerates all saved roots before checking named expectations |

See the authoritative [bounded Commit verification contract](https://github.com/memetic-activation-platform/map-dev-docs/blob/main/docs/core/testing/test-harness-design-spec.md#bounded-persisted-verification-at-commit)
and [rejection-check authoring contract](https://github.com/memetic-activation-platform/map-dev-docs/blob/main/docs/core/testing/test-step-authoring-guide.md#26-fixture-scoped-rejection-checks)
for partial subjects, inverse bindings, retries and observer semantics.

## Author and Register a Fixture

Paths below are relative to the repository root.

Start with [transaction_lifecycle_fixture.rs](../../../tests/sweetests/tests/fixture_cases/transaction_lifecycle_fixture.rs)
and its [described-instance helper](../../../tests/sweetests/tests/fixture_cases/described_instance_helpers.rs)
for schema-backed staging, Complete Commit, and saved-content comparison. Check
[adders.rs](../../../tests/sweetests/src/harness/test_case/adders.rs) for current signatures
rather than copying an API catalog.

- Build a `DancesTestCase` using `TestCaseInit` and adders. Authoring mutates in-memory expected
  snapshots, mints opaque `TestReference` tokens, and appends steps; it does not run a conductor.
- Pass returned tokens to later adders. Tokens follow their current fixture heads after Commit,
  including as relationship targets across transactions; do not construct tokens or capture
  Commit's internal result tokens. An uncommitted target from an earlier transaction fails with `CrossTransactionReference`.
- Use `add_commit_step` for create-only work. For updates, explicit dispositions, or Incomplete
  retries, read [Commit dispositions and retries](references/harness.md#commit-dispositions-and-retries)
  and the linked examples before authoring expectations.
- Expected command failures are successful test outcomes when the error kind matches.
- Exact inverse collections on shared schema holons can accumulate other fixtures' backlinks.
  Keep those inverses partial unless the scenario owns all contributors in an isolated runtime.
- Call `finalize()` exactly once after adding all steps and before returning.
- Declare the fixture module in
  [fixture_cases/mod.rs](../../../tests/sweetests/tests/fixture_cases/mod.rs), add its builder to
  the appropriate suite's `test_cases` vector in
  [dance_tests.rs](../../../tests/sweetests/tests/dance_tests.rs), and also register it in
  [fixture_authoring_tests.rs](../../../tests/sweetests/tests/fixture_authoring_tests.rs).
  Add an rstest case only for a new suite. Probe-backed scenarios stay in their isolated target.

For semantic rejection examples, inspect
[commit_validation_fixture.rs](../../../tests/sweetests/tests/fixture_cases/commit_validation_fixture.rs).
For specialized persisted graph assertions, read
[assertion freshness and graph semantics](references/harness.md#initialization-and-assertion-freshness).
`count_saved()` counts fixture-owned nodes; the count executor enumerates all saved roots,
including bootstrap and schema data. These counts are not interchangeable.

## Harness Work and Validation

For harness mechanics or debugging resolution, execution correspondence, or persisted reads,
read the relevant section of [references/harness.md](references/harness.md). Before adding a step type,
read its [adder/executor contracts](references/harness.md#adding-a-step), reuse a matching pattern,
and wire the variant in `tests/sweetests/tests/execution_steps/dance_test_runner.rs`. Put pure
classification/comparison logic in `src/harness/execution_support/` under `tests/sweetests/`
so its unit tests run once under `--lib`.

For fixture or harness authoring changes, `npm run test:unit:sweetests` runs the conductor-free
`--lib` and `fixture_authoring_tests` checks without DNA or Nix. Use `AGENTS.md` for broader
build/test entrypoints; focused conductor runs need fresh DNA/probe artifacts in Nix.
Instruction-only edits need skill, source/API, and link validation rather than conductor runs.
