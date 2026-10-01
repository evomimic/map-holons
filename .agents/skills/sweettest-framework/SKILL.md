---
name: sweettest-framework
description: >
  Orientation and authoring guide for MAP Holons integration tests (sweettests). Use this skill
  whenever a task involves writing, extending, or understanding test cases in
  tests/sweetests/ — including after completing a feature that needs test coverage, when asked
  to add a test case for a behavior, when modifying an existing fixture, or when
  a new DanceTestStep type needs to be introduced. Always consult this skill before writing
  any code in tests/sweetests/.
---

# Sweettest Framework — Authoring Guide

Use this guide to author fixtures. Read [references/harness.md](references/harness.md) when
changing harness mechanics or debugging resolution, Commit, or persisted reads. Current API
signatures live in the harness code; enduring MAP design lives in `map-dev-docs`.

## Mental Model

The framework separates fixture authoring from execution:

- **Fixture phase:** build a `DancesTestCase` with `add_*_step()` methods. Adders declare intent,
  clone and mutate in-memory expected snapshots, mint opaque `TestReference` tokens, and append
  steps. No conductor runs and nothing is persisted.
- **Execution phase:** `rstest_dance_test_suites` in `dance_tests.rs` runs registered suites via
  `run_dance_test_suite` and `run_dance_test_case` in `execution_steps/dance_test_runner.rs`.
  The expected-panic and isolated probe tests also use these drivers. Executors resolve tokens,
  dispatch `MapCommand` through `Runtime::execute_command()` and `TrustChannel`, validate, and
  record the real handles.

**Token chaining:** pass an adder's returned token into the next operation on that holon.
Adders follow its current fixture head, including after Commit; authors do not capture Commit's
internally minted result tokens.

## File Layout

```text
tests/sweetests/
  src/harness/
    test_case/adders.rs                       # authoritative adder API
    execution_support/commit_disposition.rs   # pure disposition, identity, error-delta logic
    execution_support/persisted_graph_assertions.rs # pure graph assertions
  tests/
    dance_tests.rs                            # two registered suites
    fixture_authoring_tests.rs                # conductor-free authoring gates
    commit_disposition_tests.rs               # expected-panic disposition scenarios
    commit_incomplete_tests.rs                # isolated probe-backed retries
    fixture_cases/                            # fixture builders and shared setup helpers
    execution_steps/
      dance_test_runner.rs                    # shared suite/case driver and step match arms
      persisted_read_support.rs               # assertion contexts and persisted reads
      persisted_graph_executor.rs             # declarative graph verification
      ...                                     # other step executors
```

Plain `pub fn` fixture builders are sufficient; rstest's `#[fixture]` is optional and unused
by suite registration. A file may contain several scenarios. The authoring target checks gates
without executing the runner; other direct integration targets also live in `tests/`.

## Canonical Test Case Pattern

This schema-backed example can live in `tests/fixture_cases/my_feature_fixture.rs`.
`TestCaseInit::new` returns `Self` and accepts `impl Into<String>` arguments.

```rust
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::harness::prelude::*;
use holons_prelude::prelude::*;

pub fn my_feature_fixture() -> Result<DancesTestCase, HolonError> {
    // 1. Initialize the fixture authoring state.
    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        fixture_bindings: _,
    } = TestCaseInit::new("My Feature Test", "Create and commit a described Book");

    // 2. Load the descriptor schema, then begin the instance transaction.
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;

    // 3. Schema-loaded descriptors use partial SavedLookup tokens.
    let descriptor_key = MapString(BOOK_DESCRIPTOR_KEY.into());
    let descriptor_stub = fixture_context.mutation().new_holon(Some(descriptor_key.clone()))?;
    let descriptor = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons, descriptor_stub, descriptor_key, None, None,
    )?;

    // 4. Create and stage the instance; capture tokens for chaining.
    let key = MapString("Book.MyFeature".into());
    let source = fixture_context.mutation().new_holon(Some(key.clone()))?;
    let mut properties = PropertyMap::new();
    properties.insert("Title".to_property_name(), "My Holon".to_base_value());
    let new_token = test_case.add_new_holon_step(
        &mut fixture_holons, source, properties, Some(key), None, None,
    )?;
    let staged_token = test_case.add_stage_holon_step(
        &mut fixture_holons, new_token, None, None,
    )?;
    let _described_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        staged_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![descriptor],
        None,
        None,
    )?;

    // 5. Create-only Commit advances the head; the prior token remains usable.
    test_case.add_commit_step(
        &mut fixture_holons, ExpectedCommitStatus::Complete, None, None,
    )?;
    test_case.add_match_saved_content_step()?;

    // 6. Finalize exactly once, after all steps are added.
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
```

Prefer saved-content and graph assertions to database counts. `count_saved()` counts
fixture-owned nodes, while the count executor checks all enumerated roots, including bootstrap
and schema data; these counts are not interchangeable.

### Registering a fixture

Declare `pub mod my_feature_fixture;` in `fixture_cases/mod.rs`, import the builder in
`dance_tests.rs`, and insert it into the appropriate `DanceTestSuite.test_cases` vector:

```rust
use fixture_cases::my_feature_fixture::*;

// Inside the selected suite's test_cases: vec![ ... ]:
my_feature_fixture().unwrap(),
```

Add `#[case]` in `dance_tests.rs` only for a whole new suite, not for each fixture.
Also register the builder in `fixture_authoring_tests.rs`. Keep expected-panic scenarios in
`commit_disposition_tests.rs` and probe-backed scenarios in their isolated target; never add a
probe-enabled backend to an ordinary suite.

## Choosing Adders

Consult `tests/sweetests/src/harness/test_case/adders.rs` for exact signatures. This is a starting
set, not a complete API inventory. Adders are methods on `DancesTestCase`; expected command errors
use `Option<HolonErrorKind>` and descriptions use `Option<String>`.

| Purpose | Adders | Orientation |
|---|---|---|
| Create and stage | `add_new_holon_step`, `add_stage_holon_step` | Return chainable tokens |
| Mutate content | `add_with_properties_step`, `add_remove_properties_step` | Return tokens for the updated head |
| Edit relationships | `add_add_related_holons_step`, `add_remove_related_holons_step` | Follow target heads; return updated source tokens |
| Stage updates or clones | `add_stage_new_version_step`, `add_stage_new_from_clone_step` | Clear copied predecessor; Commit establishes persisted lineage |
| Commit creates | `add_commit_step` | Create-only convenience; saved-version provenance fails at authoring time and requires explicit dispositions |
| Commit updates and retries | `add_commit_step_with_dispositions` | Declare the prepared live workset and retained relationship-retry participants |
| Check saved content | `add_match_saved_content_step` | Properties and definitional content; skips partial lookup stubs |
| Check persisted graph | `add_verify_persisted_graph_step`, `add_verify_relationship_anchoring_step` | Fresh identity/occurrence checks; the latter is the existing focused Book/Person check |
| Check rejection findings | `add_verify_commit_rejection_step`, `add_verify_commit_carrier_finding_step` | Staged semantic findings and aggregate Schema findings |
| Resolve external saved holons | `add_lookup_saved_holon_by_key_step` | Returns a partial SavedLookup token; use for schema-loaded holons outside the fixture ledger |
| Manage lifecycle | `add_begin_transaction_step`, `add_abandon_staged_changes_step`, `add_delete_holon_step` | Explicit transactions, abandon, and deletion |
| Load or inspect | `add_load_book_person_inverse_test_schema_step`, `add_load_core_schema_step`, `add_load_holons_internal_step`, `add_query_relationships_step`, `add_database_print_step` | See source and matching fixtures for expected counts and outcomes |
| Count enumerated roots | `add_ensure_database_count_step` | Illustrative API, unused by current fixtures; not a fixture-ownership count |

## Declaring Commit dispositions

For Complete or supported Incomplete attempts, declare what each prepared live Pass 1 candidate
should do. These are verbatim expectations, never inferred from fixture mutations or the response:

- `NewRoot` — a create or independent clone, with no inherited lineage.
- `NoAction` — an unchanged update: nothing is saved, and the token binds to the saved source.
- `GraphOnly` — graph writes anchored to the source identity, whose lineage is untouched.
- `NewVersion` — a new identity whose single predecessor is the staging source.

Use `ExpectedCommitCandidate::new(token, disposition)` for every live candidate exactly once.
Mutations never rewrite a declaration: a property write can promote a graph-only candidate to
NewVersion, so a stale GraphOnly expectation fails as a disposition mismatch before snapshots.
NoAction saves nothing, but its token remains usable as the saved source.

Operational errors are separate from command errors and semantic findings. Attach
`.with_expected_new_errors(...)` only when needed; omission expects none. Checks compare newly
appended occurrences against a per-attempt baseline, including kind and multiplicity.

A retained committed entry retrying only relationships uses `ExpectedRetryParticipant`, with no
disposition or new saved result. Each attempt needs fresh expectations, even with zero live
candidates. Supported Incomplete means successful Pass 1 followed by relationship failure;
a failure before a Saved outcome is unsupported, not NoAction, and says nothing about whether
a node write occurred. See the worked examples below and the outcome table in
[references/harness.md](references/harness.md#commit-dispositions-and-retries).

## Key Rules for Test Authors

- Treat `TestReference` as an opaque immutable token; pass it to adders rather than constructing it.
- Existing tokens follow their fixture heads after Commit, including as relationship targets
  across transactions. An uncommitted target from an earlier transaction fails add/remove with
  `CrossTransactionReference` naming the relationship and target.
- `Rejected` attempts and commits with `expected_error` take no candidate or retry declarations
  and advance no heads. Use existing rejection-finding expectations for semantic failures.
- Use `add_match_saved_content_step()` after Commit for saved properties and definitional
  content, including after a version-producing Commit. Inverses, non-definitional graph changes
  and exact lineage need `add_verify_persisted_graph_step()` instead.
- Expected command failures are successful test outcomes when the error kind matches.
- Call `finalize()` exactly once, after adding all steps and before returning.

## Existing Fixtures (Model From These)

Start with the closest example in `tests/sweetests/tests/fixture_cases/`, rather than copying
an unrelated fixture:

| Fixture | Use it for |
|---|---|
| `transaction_lifecycle_fixture.rs`, `described_instances.rs` | The shortest happy path: schema-backed staging, a Complete Commit, and saved-content comparison |
| `stage_new_version_fixture.rs`, `stage_new_from_clone_fixture.rs` | Update/clone staging and existing anchoring checks |
| `commit_disposition_fixture.rs` | Explicit dispositions, zero-save updates, same-key identities, negative diagnostics |
| `commit_lineage_fixture.rs` | Sequential lineage, non-root updates, graph replay, independent clones |
| `commit_competition_fixture.rs` | Rejection/correction/retry and branches across transactions |
| `commit_incomplete_fixture.rs` + `commit_incomplete_tests.rs` | Repeated relationship failures; injection stays in the isolated test target |
| `simple_create_holon_fixture.rs`, `commit_validation_fixture.rs`, `commit_strict_contract_fixture.rs`, `commit_schema_fixture.rs` | Semantic findings and rejection contracts; undescribed creation is rejected |
| `load_book_person_inverse_schema_fixture.rs` | Descriptor/instance links and head redirection across transactions |
| `transaction_lifecycle_fixture.rs`, `delete_holon_fixture.rs`, `abandon_staged_changes_fixture.rs` | Multi-transaction lifecycle, deletion, abandon, and expected failures |

## Running Tests

```bash
# Inner loop: --lib + fixture_authoring_tests; no conductor, DNA, or Nix.
npm run test:unit:sweetests

# Broader Rust/TypeScript unit checks.
npm run test:unit

# Completion gate: full Sweettest suite in Nix.
nix develop
npm install
npm run sweetest
exit
```

The harness inner loop is sub-second after compilation. Full Sweettest takes over an hour:
use it as a completion gate. For focused integration checks in Nix, build fresh DNA/probe artifacts
and select a target with `cargo test --manifest-path tests/sweetests/Cargo.toml --test <target>`.
`npm run sweetest:nocapture` enables full output.

## When a New Step Type Is Needed

If no existing adder covers the behavior, add an adder and executor and wire the new variant in
`execution_steps/dance_test_runner.rs`. Read the adder/executor contracts in
[references/harness.md](references/harness.md) first. Put pure classification or comparison
logic and its unit tests in `src/harness/execution_support/` so those tests run once under `--lib`.
