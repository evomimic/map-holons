//! # Holon Loader Test Fixtures
//!
//! This module provides rstest fixtures for testing the holon loader's two-pass workflow.
//!
//! ## Fixture Progression
//!
//! 1. **`loader_minimal_fixture`**: Empty bundle + nodes-only (no relationships)
//! 2. **`load_holons_declared_links_fixture`**: Declared relationship (forward direction)
//! 3. *[Future]* Inverse, error handling, deduplication, and large-scale fixtures
//!
//! ## Key Implementation Notes
//!
//! - `create_empty(key)` sets the holon's internal identifier, but LoaderHolons must
//!   **explicitly set their `Key` property** for Pass-1 resolution
//! - Type descriptors are not required for basic declared relationship testing
//! - Endpoints are resolved by key during Pass-2 via Nursery lookup

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
    let transient_service_handle = context.get_space_manager().get_transient_behavior_service();

    let transient_service = transient_service_handle
        .write()
        .map_err(|_| HolonError::FailedToBorrow("Transient service lock was poisoned".into()))?;

    let bundle = transient_service.create_empty(MapString(bundle_key.into()))?;
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
    let transient_service_handle = context.get_space_manager().get_transient_behavior_service();

    let transient_service = transient_service_handle
        .write()
        .map_err(|_| HolonError::FailedToBorrow("Transient service lock was poisoned".into()))?;

    // 1) Create the bundle container.
    let mut bundle = transient_service.create_empty(MapString(bundle_key.into()))?;

    // 2) Create LoaderHolon containers (minimal: just a Key property).
    let mut members: Vec<HolonReference> = Vec::with_capacity(node_keys.len());
    for key in node_keys {
        let loader_node =
            transient_service.create_empty(MapString(format!("LoaderHolon.{key}")))?;
        members.push(HolonReference::Transient(loader_node));
    }
    drop(transient_service); // release lock before adding relationships

    // 3) Attach members via BUNDLE_MEMBERS.
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers.as_relationship_name().clone(),
        members,
    )?;

    Ok((bundle, node_keys.len()))
}

/// Build a HolonLoaderBundle with:
///   - Two LoaderHolons representing instance nodes (source, target)
///   - One declared LoaderRelationshipReference from source → target
///
/// This exercises:
///   - Pass-1: stage two instance holons (properties-only)
///   - Pass-2: create **one** declared link (LinksCreated = 1)
///
/// RETURNS: (bundle, expected_node_count, expected_links_created)
fn build_declared_links_bundle(
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
    source_instance_key: &str,
    target_instance_key: &str,
    declared_relationship_name: &str,
) -> Result<(TransientReference, usize, usize), HolonError> {
    use holons_prelude::prelude::*;

    // 1) Create the bundle container.
    let transient_service_handle = context.get_space_manager().get_transient_behavior_service();
    let mut transient_service = transient_service_handle
        .write()
        .map_err(|_| HolonError::FailedToBorrow("Transient service lock was poisoned".into()))?;
    let mut bundle = transient_service.create_empty(MapString(bundle_key.into()))?;

    // 2) Create minimal LoaderHolons for the source and target instances.
    //    (You can add properties here if you want to assert propagation later.)
    let mut source_loader = transient_service
        .create_empty(MapString(format!("LoaderHolon.{source}", source = source_instance_key)))?;
    let target_loader = transient_service
        .create_empty(MapString(format!("LoaderHolon.{target}", target = target_instance_key)))?;
    // Done with transient service for now.
    drop(transient_service);

    // 3) Attach both as BundleMembers.
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers.as_relationship_name().clone(),
        vec![
            HolonReference::Transient(source_loader.clone()),
            HolonReference::Transient(target_loader.clone()),
        ],
    )?;

    // 4) Add a **declared** LoaderRelationshipReference on the source pointing to the target.
    add_declared_relationship_reference(
        context,
        &mut source_loader,
        declared_relationship_name,
        source_instance_key,
        &[target_instance_key],
    )?;

    Ok((bundle, 2, 1))
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
    // test_case.add_database_print_step()?; // problem with client fetch_holon_internal()

    // Export the fixture’s transient pool into the test case’s session state.
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case)
}

// ─────────────────────────────────────────────────────────────────────────────
// Placeholders for possible upcoming fixtures (keep in this file, or split to subfiles
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

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers for loader holon building
// ─────────────────────────────────────────────────────────────────────────────

/// Build and attach a **declared** LoaderRelationshipReference (LRR) to a given LoaderHolon,
/// wiring up its ReferenceSource and ordered ReferenceTarget(s).
///
/// This function **only** constructs the *loader-side* graph:
/// - `HasRelationshipReference`: LoaderHolon → LoaderRelationshipReference
/// - `ReferenceSource`: LRR → LoaderHolonReference (source)
/// - `ReferenceTarget`: LRR → LoaderHolonReference(s) (targets, kept in order)
///
/// At load time, the resolver uses:
///   - `relationship_name` (declared side),
///   - endpoint keys (via LoaderHolonReference.holon_key),
/// to write the real links on the staged instance holons.
///
/// RETURNS: the created LoaderRelationshipReference transient reference.
fn add_declared_relationship_reference(
    context: &dyn HolonsContextBehavior,
    source_loader_holon: &mut TransientReference,
    relationship_name_str: &str,
    source_instance_key: &str,
    target_instance_keys: &[&str],
) -> Result<TransientReference, HolonError> {
    use holons_prelude::prelude::*;

    // Acquire the transient service so we can create the LRR + endpoint references.
    let transient_service_handle = context.get_space_manager().get_transient_behavior_service();
    let mut transient_service = transient_service_handle
        .write()
        .map_err(|_| HolonError::FailedToBorrow("Transient service lock was poisoned".into()))?;

    // 1) Create the LoaderRelationshipReference container.
    let relationship_reference_key = format!(
        "LoaderRelationshipReference.{}.{relationship_name}",
        source_instance_key,
        relationship_name = relationship_name_str
    );
    let mut relationship_reference =
        transient_service.create_empty(MapString(relationship_reference_key))?;

    // 2) Set required properties on the LRR.
    relationship_reference.with_property_value(
        context,
        CorePropertyTypeName::RelationshipName.as_property_name(),
        BaseValue::StringValue(MapString(relationship_name_str.to_string())),
    )?;
    relationship_reference.with_property_value(
        context,
        CorePropertyTypeName::IsDeclared.as_property_name(),
        BaseValue::BooleanValue(MapBoolean(true)),
    )?;

    // 3) Create the LoaderHolonReference for the source endpoint (by local key).
    let source_ref_key = format!("LoaderHolonReference.Source.{}", source_instance_key);
    let mut source_ref = transient_service.create_empty(MapString(source_ref_key))?;
    source_ref.with_property_value(
        context,
        CorePropertyTypeName::HolonKey.as_property_name(),
        BaseValue::StringValue(MapString(source_instance_key.to_string())),
    )?;

    // 4) Create ordered LoaderHolonReference(s) for each target endpoint (by local key).
    let mut target_refs: Vec<HolonReference> = Vec::with_capacity(target_instance_keys.len());
    for (index, target_key) in target_instance_keys.iter().enumerate() {
        let target_ref_key = format!("LoaderHolonReference.Target{}.{}", index + 1, target_key);
        let mut target_ref = transient_service.create_empty(MapString(target_ref_key))?;
        target_ref.with_property_value(
            context,
            CorePropertyTypeName::HolonKey.as_property_name(),
            BaseValue::StringValue(MapString((*target_key).to_string())),
        )?;
        target_refs.push(HolonReference::Transient(target_ref));
    }

    // Release the lock before we start adding relationships.
    drop(transient_service);

    // 5) Attach the LRR to the LoaderHolon (HasRelationshipReference).
    source_loader_holon.add_related_holons(
        context,
        CoreRelationshipTypeName::HasRelationshipReference.as_relationship_name().clone(),
        vec![HolonReference::Transient(relationship_reference.clone())],
    )?;

    // 6) Wire ReferenceSource (exactly one) and ordered ReferenceTarget(s).
    relationship_reference.add_related_holons(
        context,
        CoreRelationshipTypeName::ReferenceSource.as_relationship_name().clone(),
        vec![HolonReference::Transient(source_ref)],
    )?;
    relationship_reference.add_related_holons(
        context,
        CoreRelationshipTypeName::ReferenceTarget.as_relationship_name().clone(),
        target_refs,
    )?;

    Ok(relationship_reference)
}
