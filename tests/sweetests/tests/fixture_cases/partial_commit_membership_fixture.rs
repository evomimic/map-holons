//! Regression fixture: a holon that publishes stays discoverable even when a sibling does not.
//!
//! Whole-space discovery reads the current space's `Owns` members, so a published holon that never
//! acquired membership is persisted and permanently invisible. The commit's relationship pass is
//! skipped outright whenever *any* staged holon fails to publish, which is precisely the case this
//! fixture builds — so membership authored there would not survive it. Membership is instead
//! authored in the node pass, beside `persist_holon`, and this pins that.
//!
//! Shape: two holons staged in one commit. One carries more properties than PVL permits, so its
//! `PublishRoot` is rejected and the commit reports `Incomplete` before the relationship pass runs.
//! The other publishes normally and must still be an owned member afterwards.

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
        "Stage one publishable holon and one that PVL rejects, commit both, and assert the \
         publishable one is still an owned member of the space after the relationship pass is \
         skipped.",
    );

    // Start from an empty space so the count below is unambiguous.
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
        Some(bad_key),
        None,
        Some("Creating the holon that exceeds the PVL property limit...".to_string()),
    )?;
    test_case.add_stage_holon_step(
        &mut fixture_holons,
        bad_token,
        None,
        Some("Staging the holon that will be rejected...".to_string()),
    )?;

    // Pass 1 fails for the over-limit holon, so the commit reports Incomplete and the
    // relationship pass never runs.
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Incomplete,
        None,
        Some("Committing both; expect Incomplete".to_string()),
    )?;

    // The assertion this fixture exists for. A literal count rather than `count_saved()`: the
    // fixture ledger advances every staged head to Saved on commit, which is right when only the
    // relationship pass fails, but overcounts here because one holon never published at all.
    test_case.add_ensure_database_count_step(
        MapInteger(1),
        Some("The published holon is still an owned member after a partial commit".to_string()),
    )?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
