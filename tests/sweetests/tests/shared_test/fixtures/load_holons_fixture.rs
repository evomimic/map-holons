use holons_prelude::prelude::*;
use rstest::*;

use crate::shared_test::{test_context::init_fixture_context, test_data_types::DancesTestCase};

// ─────────────────────────────────────────────────────────────────────────────
// Internal bundle builders (kept private to this file)
// ─────────────────────────────────────────────────────────────────────────────

/// Build a HolonLoaderBundle **without** any BUNDLE_MEMBERS.
/// This intentionally exercises the “empty bundle” short-circuit path.
fn build_empty_bundle(
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
) -> Result<TransientReference, HolonError> {
    let transient_service = context.get_space_manager().get_transient_behavior_service();
    let bundle = transient_service.borrow().create_empty(MapString(bundle_key.into()))?;
    Ok(bundle)
}

/// Build a HolonLoaderBundle with **N minimal LoaderHolons (nodes only)**.
/// Each member has a Key property and **no** relationship references.
/// This exercises Pass-1 staging + commit, with LinksCreated = 0.
fn build_nodes_only_bundle(
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
    node_keys: &[&str],
) -> Result<(TransientReference, usize), HolonError> {
    let transient_service = context.get_space_manager().get_transient_behavior_service();
    let borrowed = transient_service.borrow();

    // 1) Create the bundle container.
    let bundle = borrowed.create_empty(MapString(bundle_key.into()))?;

    // 2) Create LoaderHolon containers (minimal: just a Key property).
    let mut members: Vec<HolonReference> = Vec::with_capacity(node_keys.len());
    for key in node_keys {
        let loader_node = borrowed.create_empty(MapString(format!("LoaderHolon.{key}")))?;
        // loader_node.with_property_value(
        //     context,
        //     CorePropertyTypeName::Key.as_property_name(),
        //     BaseValue::StringValue(MapString((*key).into())),
        // )?;
        members.push(HolonReference::Transient(loader_node));
    }

    // 3) Attach members via BUNDLE_MEMBERS.
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers.as_relationship_name().clone(),
        members,
    )?;

    Ok((bundle, node_keys.len()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Public fixtures (each returns a complete DancesTestCase)
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal loader path:
///  1) Empty bundle → UnprocessableEntity; DB remains 1 (space holon)
///  2) Nodes-only bundle (3 nodes) → OK; LinksCreated=0; DB becomes 1 + 3
#[fixture]
pub fn loader_minimal_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Loader Minimal Fixture".to_string(),
        "Empty bundle (422) then nodes-only (OK)".to_string(),
    );

    // Create a private fixture context with its own TransientHolonManager.
    let fixture_context = init_fixture_context();

    // A) Ensure DB starts with only the Space holon.
    test_case.add_ensure_database_count_step(MapInteger(1))?;

    // B) Empty bundle → expect UnprocessableEntity and no DB change.
    let empty_bundle = build_empty_bundle(&*fixture_context, "Bundle.Empty.1")?;
    test_case.add_load_holons_step(
        empty_bundle,
        ResponseStatusCode::UnprocessableEntity,
        MapInteger(0), // HolonsStaged
        MapInteger(0), // HolonsCommitted
        MapInteger(0), // LinksCreated
        MapInteger(0), // ErrorCount
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1))?;

    // C) Nodes-only bundle → expect OK, N committed, 0 links created.
    let (nodes_bundle, n) = build_nodes_only_bundle(
        &*fixture_context,
        "Bundle.NodesOnly.1",
        &["Book.TheHollowTree", "Person.AMonk", "Publisher.GreenLeaf"],
    )?;
    test_case.add_load_holons_step(
        nodes_bundle,
        ResponseStatusCode::OK,
        MapInteger(n as i64), // HolonsStaged
        MapInteger(n as i64), // HolonsCommitted
        MapInteger(0),        // LinksCreated
        MapInteger(0),        // ErrorCount
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1 + n as i64))?;
    (test_case).add_database_print_step()?;

    // Export the fixture’s transient pool into the test case’s session state.
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case)
}

// ─────────────────────────────────────────────────────────────────────────────
// Placeholders for upcoming fixtures (keep in this file, or split to subfiles
// once they grow):
//   #[fixture] pub fn load_holons_declared_links_fixture() -> Result<DancesTestCase, HolonError> { … }
//   #[fixture] pub fn load_holons_inverse_links_fixture() -> Result<DancesTestCase, HolonError> { … }
//   #[fixture] pub fn load_holons_pass1_error_fixture() -> Result<DancesTestCase, HolonError> { … }
//   #[fixture] pub fn load_holons_pass2_error_fixture() -> Result<DancesTestCase, HolonError> { … }
//   #[fixture] pub fn load_holons_dedupe_fixture() -> Result<DancesTestCase, HolonError> { … }
//   #[fixture] pub fn load_holons_saved_staged_mix_fixture() -> Result<DancesTestCase, HolonError> { … }
//   #[fixture] pub fn load_holons_by_id_fixture() -> Result<DancesTestCase, HolonError> { … }
//   #[fixture] #[ignore] pub fn load_holons_large_smoke_fixture() -> Result<DancesTestCase, HolonError> { … }
// ─────────────────────────────────────────────────────────────────────────────
