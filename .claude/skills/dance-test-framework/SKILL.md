---
name: dance-test-framework
description: >
  Orientation and authoring guide for MAP Holons integration tests (sweettests). Use this skill
  whenever a task involves writing, extending, or understanding test cases in
  tests/sweetests/ — including after completing a feature that needs test coverage, when asked
  to add a test case for a behavior, when reading or modifying an existing fixture, or when
  a new DanceTestStep type needs to be introduced. Always consult this skill before writing
  any code in tests/sweetests/.
---

# Dance Test Framework — Authoring Guide

Read `references/harness.md` for deep harness mechanics (TestReference internals, FixtureHolon
lifecycle, commit token minting, executor invariants). This file covers everything needed to
write a new test case or extend an existing one.

---

## Mental Model

The framework has two completely separate phases:

**Fixture Phase** (test case definition, no execution)
- You construct a `DancesTestCase` by calling `add_*_step()` methods.
- Each adder declares *intent* and *expected outcome*, mints `TestReference` tokens, and appends
  a `DanceTestStep` to the case.
- No Holochain conductor, no real holon operations. Pure specification.

**Execution Phase** (real dispatch through conductor → WASM → DHT)
- `rstest_dance_tests` in `dance_tests.rs` drives this automatically.
- Each step is dispatched as a `MapCommand` through `Runtime::execute_command()`, which calls
  the real Holochain conductor via `TrustChannel`.
- Executors resolve `TestReference` tokens to runtime holon handles, execute, validate, record.

**Token chaining** is the core authoring pattern: every adder that affects a holon returns a
`TestReference`. Pass that token into the next adder that operates on the same holon. The harness
internally handles head-following after commit — you never need to update your token.

---

## File Layout

```
tests/sweetests/
  src/harness/           ← harness internals (don't modify unless adding a new step type)
  tests/
    dance_tests.rs       ← registers all test cases; add #[case] here
    fixture_cases/       ← one file per test case; add new fixtures here
    execution_steps/     ← one executor per step type (modify only when adding new step types)
```

---

## Canonical Test Case Pattern

```rust
use holons_test::harness::prelude::*;  // TestCaseInit, DancesTestCase, FixtureHolons, etc.
use holons_prelude::prelude::*;
use integrity_core_types::HolonErrorKind;

#[fixture]
pub fn my_feature_fixture() -> Result<DancesTestCase, HolonError> {
    // 1. Initialize — always destructure like this
    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        fixture_bindings: _,
    } = TestCaseInit::new(
        "My Feature Test".to_string(),
        "Brief description of what this test exercises".to_string(),
    )?;

    // 2. Optional: assert DB starts empty
    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), None)?;

    // 3. Create a transient holon in the fixture context (needed for NewHolon step)
    let my_holon_ref = fixture_context.mutation().new_holon(Some(MapString("my-key".into())))?;

    // 4. Build a property map for the NewHolon step
    let mut properties = PropertyMap::new();
    properties.insert("title".to_property_name(), MapString("My Holon".into()).to_base_value());

    // 5. Add steps — capture returned tokens when you need to chain
    let new_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        my_holon_ref,
        properties,
        Some(MapString("my-key".into())),  // key
        None,                               // expected_error (None = expect success)
        None,                               // description (None = use default)
    )?;

    let staged_token = test_case.add_stage_holon_step(
        &mut fixture_holons,
        new_token,
        None,
        None,
    )?;

    // 6. Commit — no return value; staged_token remains valid
    test_case.add_commit_step(&mut fixture_holons, None, None)?;

    // 7. Assertions
    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), None)?;
    test_case.add_match_saved_content_step()?;

    // 8. Finalize — required; no more steps after this
    test_case.finalize(&fixture_context)?;
    Ok(test_case)
}
```

### Registering the new test case in `dance_tests.rs`

```rust
// At the top, add the import:
use fixture_cases::my_feature_fixture::*;

// Add a #[case] line to rstest_dance_tests:
#[case::my_feature_test(my_feature_fixture())]
```

---

## Adder Reference

All adders are methods on `DancesTestCase`. `expected_error: Option<HolonErrorKind>` is `None`
for success cases. A step expected to fail and that does fail is a **successful test outcome** —
execution continues normally.

| Adder | Returns token? | Notes |
|---|---|---|
| `add_new_holon_step(holons, source_ref, properties, key, err, desc)` | yes | Creates a new transient holon. `source_ref` comes from `fixture_context.mutation().new_holon(...)` |
| `add_stage_holon_step(holons, token, err, desc)` | yes | Stages the transient holon identified by `token` |
| `add_with_properties_step(holons, token, properties, err, desc)` | yes | Sets/overwrites properties on the holon |
| `add_remove_properties_step(holons, token, properties, err, desc)` | yes | Removes named properties |
| `add_add_related_holons_step(holons, token, rel_name, Vec<TestReference>, err, desc)` | yes | Adds relationship targets |
| `add_remove_related_holons_step(holons, token, rel_name, Vec<TestReference>, err, desc)` | yes | Removes relationship targets |
| `add_commit_step(holons, err, desc)` | no | Commits all staged holons; advances heads internally |
| `add_abandon_staged_changes_step(holons, token, err, desc)` | yes | Abandons staged state for one holon |
| `add_delete_holon_step(holons, token, err, desc)` | no | Deletes a saved holon |
| `add_stage_new_version_step(holons, token, err, version_count, staging_err, desc)` | yes | Stages a new version; auto-adds Predecessor relationship |
| `add_stage_new_from_clone_step(holons, token, new_key, err, desc)` | yes | Clones an existing holon with a new key |
| `add_query_relationships_step(holons, token, query_expr, err, desc)` | no | Queries relationships (no content comparison yet) |
| `add_begin_transaction_step(err, desc)` | no | Begins a new transaction explicitly |
| `add_commit_step(holons, err, desc)` | no | (same as above) |
| `add_ensure_database_count_step(expected_count, desc)` | no | Asserts saved holon count = `fixture_holons.count_saved()` |
| `add_match_saved_content_step()` | no | Asserts all saved holons match their fixture-time snapshots |
| `add_database_print_step()` | no | Debug: prints the current DB state |
| `add_load_core_schema_step(desc)` | no | Loads MAP core schema |
| `add_load_book_person_inverse_test_schema_step(desc)` | no | Loads book/person inverse test schema |
| `add_load_holons_internal_step(set_ref, staged, committed, links, errors, bundles, loaders)` | no | Loads holons from an import set |

---

## Key Rules for Test Authors

- **Don't inspect `TestReference`** — treat it as an opaque handle.
- **After `add_commit_step`**, existing tokens remain valid; the harness follows the new head
  automatically. Don't try to capture new tokens from commit.
- **Use `fixture_holons.count_saved()`** in `add_ensure_database_count_step` — don't count manually.
- **`add_match_saved_content_step()`** is the primary content assertion; use it after commit.
- **Expected failures are first-class**: pass `Some(HolonErrorKind::...)` as `expected_error`.
  The test continues normally when the step fails as expected.
- **`finalize()` must be called exactly once**, after all steps are added and before returning.

---

## Existing Fixtures (Model From These)

| Fixture file | What it tests |
|---|---|
| `simple_create_holon_fixture.rs` | Create, stage, commit, assert DB count and content |
| `simple_add_remove_properties_fixture.rs` | Add/remove properties pre- and post-commit |
| `simple_add_remove_related_holons_fixture.rs` | Add/remove relationship targets |
| `ergonomic_add_remove_properties_fixture.rs` | Same as above, ergonomic adder API |
| `ergonomic_add_remove_related_holons_fixture.rs` | Same as above, ergonomic adder API |
| `stage_new_from_clone_fixture.rs` | Clone an existing holon with a new key |
| `stage_new_version_fixture.rs` | Stage a new version with Predecessor relationship |
| `abandon_staged_changes_fixture.rs` | Abandon staged state |
| `delete_holon_fixture.rs` | Delete a saved holon, including expected-failure case |
| `transaction_lifecycle_fixture.rs` | Explicit begin/commit transaction lifecycle |
| `load_core_schema_fixture.rs` | Load MAP core schema |

**Before writing a new test case**, read the most relevant existing fixture. The pattern is
consistent — studying one complete example is the fastest orientation.

---

## Running Tests

```bash
# Unit tests (fast, no Nix required)
npm run test:unit

# Full sweettest suite (requires Nix shell)
nix develop
npm run sweetest

# With full output
npm run sweetest:nocapture
```

---

## When a New Step Type Is Needed

If no existing adder covers the behavior you need, you must add both an adder and an executor.
Read `references/harness.md` — specifically the "Test Step Authoring Guide" section — before
writing either. The canonical adder and executor sequences are non-negotiable.
