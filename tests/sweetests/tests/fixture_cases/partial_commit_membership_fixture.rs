//! Membership across an incomplete commit and the follow-up that completes it.
//!
//! A Pass 1 failure does not finalize the transaction: `should_transition_from_commit_response`
//! returns `false` for `Incomplete`, so the transaction stays open and its nursery is retained.
//! A holon that did publish is held as a staged holon in `Committed(saved_id)` state, and its
//! relationships are persisted by the *next* complete commit, which reaches it through that
//! retained staged reference.
//!
//! Whole-space discovery is a finalized persisted-membership view, not the recovery surface for a
//! partially completed transaction, so this fixture asserts both halves of that contract:
//!
//! 1. After `Incomplete`, the published holon is **not** yet an owned member — `GetAllHolons` does
//!    not expose unfinished transaction state.
//! 2. After the follow-up complete commit, it **is**.
//!
//! Assertion 2 is also what proves the holon stayed addressable through its staged reference:
//! nothing else can write its membership. Pass 1 treats it as `NoAction` (already `Committed`), so
//! the only route to its `OwnedBy` collection is `RelationshipCommitSource::from_committed_staged_holon`
//! resolving the reference the nursery retained. Had that been dropped, the count would stay 0.
//!
//! Shape: two holons staged together. One carries more properties than PVL permits, so its
//! `PublishRoot` is rejected and the commit reports `Incomplete` before the relationship pass runs.
//! It is then abandoned and the transaction committed again.

use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};
use rstest::*;
use std::collections::BTreeMap;

/// One more than `shared_validation::MAX_PROPERTY_COUNT`, so `preflight_holon_node` rejects the
/// node before it is authored. Any Pass 1 failure would do; this one needs no oversized payload.
const OVER_PROPERTY_LIMIT: usize = 257;

#[fixture]
pub fn partial_commit_membership_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        fixture_bindings: _fixture_bindings,
    } = TestCaseInit::new(
        "Partial Commit Membership Testcase",
        "Stage one publishable holon and one that PVL rejects; assert the publishable one is not \
         an owned member while the commit is Incomplete, and becomes one once the transaction is \
         completed.",
    );

    // Start from an empty space so the counts below are unambiguous.
    test_case.add_ensure_database_count_step(
        MapInteger(0),
        Some("DB starts with no owned holons".to_string()),
    )?;

    // ── The holon that publishes ────────────────────────────────────────────────
    let good_key = MapString("partial-commit:publishable".to_string());
    let good_transient = fixture_context.mutation().new_holon(Some(good_key.clone()))?;
    let mut good_properties = BTreeMap::new();
    good_properties.insert("title".to_property_name(), "Publishable Holon".to_base_value());

    let good_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        good_transient,
        good_properties,
        Some(good_key),
        None,
        Some("Creating the holon that will publish...".to_string()),
    )?;
    test_case.add_stage_holon_step(
        &mut fixture_holons,
        good_token,
        None,
        Some("Staging the holon that will publish...".to_string()),
    )?;

    // ── The holon that PVL rejects ──────────────────────────────────────────────
    let bad_key = MapString("partial-commit:rejected".to_string());
    let bad_transient = fixture_context.mutation().new_holon(Some(bad_key.clone()))?;
    let mut bad_properties = BTreeMap::new();
    for index in 0..OVER_PROPERTY_LIMIT {
        bad_properties
            .insert(format!("property-{index:03}").to_property_name(), "value".to_base_value());
    }

    let bad_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        bad_transient,
        bad_properties,
        Some(bad_key.clone()),
        None,
        Some("Creating the holon that exceeds the PVL property limit...".to_string()),
    )?;
    let bad_staged_token = test_case.add_stage_holon_step(
        &mut fixture_holons,
        bad_token,
        None,
        Some("Staging the holon that will be rejected...".to_string()),
    )?;

    // Pass 1 fails for the over-limit holon, so the commit reports Incomplete and the
    // relationship pass never runs.
    test_case.add_commit_step_expecting_unsaved(
        &mut fixture_holons,
        &[bad_key.clone()],
        ExpectedCommitStatus::Incomplete,
        None,
        Some("Committing both; expect Incomplete".to_string()),
    )?;

    // The transaction is not finished, so its published holon is not yet an owned member. A
    // literal count rather than `count_saved()`: the fixture ledger advances every staged head to
    // Saved on commit, which is right once the transaction completes but not while it is open.
    test_case.add_ensure_database_count_step(
        MapInteger(0),
        Some("Incomplete commit is not exposed through whole-space discovery".to_string()),
    )?;

    // Repair the transaction by abandoning the holon that could not be published.
    test_case.add_abandon_staged_changes_step(
        &mut fixture_holons,
        bad_staged_token,
        None,
        Some("Abandoning the rejected holon".to_string()),
    )?;

    // Pass 1 now reports NoAction for the already-published holon and Abandoned for the other, so
    // the commit completes and the relationship pass persists membership from the retained
    // staged reference.
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Committing again; expect Complete".to_string()),
    )?;

    test_case.add_ensure_database_count_step(
        MapInteger(1),
        Some("Completing the transaction makes the published holon an owned member".to_string()),
    )?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
