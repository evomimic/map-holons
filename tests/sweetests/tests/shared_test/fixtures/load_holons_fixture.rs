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
//! - `create_empty(key: String | MapString)` **sets the holon `Key` property automatically**,
//!   so we simply pass the *intended instance key string* when creating LoaderHolons.
//! - Type descriptors are **not** required for these fixtures.
//! - Pass-2 resolves LoaderRelationshipReference endpoints by `LoaderHolonReference.holon_key`,
//!   which must match the LoaderHolon keys we pass to `create_empty()`.
//!
//! Result: endpoint resolution uses consistent strings everywhere.

use holons_prelude::prelude::*;
use rstest::*;

use crate::shared_test::test_data_types::{BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY};
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

    // `create_empty()` sets the bundle's `Key` to the provided string.
    let bundle = transient_service.create_empty(MapString(bundle_key.to_string()))?;
    Ok(bundle)
}

/// Build a HolonLoaderBundle with **N minimal LoaderHolons (nodes only)**.
/// Each member is created with its **instance key string**; no relationship references.
/// This exercises Pass-1 staging + commit with LinksCreated = 0.
fn build_nodes_only_bundle(
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
    instance_keys: &[&str],
) -> Result<(TransientReference, usize), HolonError> {
    let transient_service_handle = context.get_space_manager().get_transient_behavior_service();
    let transient_service = transient_service_handle
        .write()
        .map_err(|_| HolonError::FailedToBorrow("Transient service lock was poisoned".into()))?;

    // 1) Create the bundle container (Key = bundle_key).
    let mut bundle = transient_service.create_empty(MapString(bundle_key.to_string()))?;

    // 2) Create LoaderHolon containers with **consistent instance keys**.
    //    `create_empty()` sets the Key property; no manual Key setting required.
    let mut members: Vec<HolonReference> = Vec::with_capacity(instance_keys.len());
    for key in instance_keys {
        let loader_holon = transient_service.create_empty(MapString((*key).to_string()))?;
        members.push(HolonReference::Transient(loader_holon));
    }
    drop(transient_service); // release lock before mutating relationships

    // 3) Attach members via BUNDLE_MEMBERS.
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers.as_relationship_name().clone(),
        members,
    )?;

    Ok((bundle, instance_keys.len()))
}

/// Build a HolonLoaderBundle with:
///   - Two LoaderHolons representing instances (source, target)
///   - One declared LoaderRelationshipReference from source → target
///
/// All keys are **consistent**:
///   - Source LoaderHolon: `create_empty(source_instance_key)`
///   - Target LoaderHolon: `create_empty(target_instance_key)`
///   - LoaderHolonReference.holon_key values use the **same strings**
///
/// RETURNS: (bundle, expected_node_count, expected_links_created)
fn build_declared_links_bundle(
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
    source_instance_key: &str,
    target_instance_key: &str,
    declared_relationship_name: &str,
) -> Result<(TransientReference, usize, usize), HolonError> {
    // 1) Create the bundle container (Key = bundle_key).
    let transient_service_handle = context.get_space_manager().get_transient_behavior_service();
    let mut transient_service = transient_service_handle
        .write()
        .map_err(|_| HolonError::FailedToBorrow("Transient service lock was poisoned".into()))?;

    // LoaderHolons created with **instance key strings** (Key set automatically).
    let mut bundle = transient_service.create_empty(MapString(bundle_key.to_string()))?;
    let mut source_loader =
        transient_service.create_empty(MapString(source_instance_key.to_string()))?;
    let target_loader =
        transient_service.create_empty(MapString(target_instance_key.to_string()))?;
    drop(transient_service); // release lock before wiring relationships

    // 2) Attach both as BundleMembers.
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers.as_relationship_name().clone(),
        vec![
            HolonReference::Transient(source_loader.clone()),
            HolonReference::Transient(target_loader.clone()),
        ],
    )?;

    // 3) Add a **declared** LRR on the source pointing to the target (by matching holon_key).
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
pub async fn loader_minimal_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Loader Minimal Fixture".to_string(),
        "Empty bundle (422) then nodes-only (OK)".to_string(),
    );

    // Create a private fixture context with its own TransientHolonManager.
    let fixture_context_arc = init_fixture_context().await;
    let fixture_context_ref: &dyn HolonsContextBehavior = &*fixture_context_arc;

    // A) Ensure DB starts with only the Space holon.
    test_case.add_ensure_database_count_step(MapInteger(1))?;

    // B) Empty bundle → expect UnprocessableEntity and no DB change.
    let empty_bundle = build_empty_bundle(fixture_context_ref, "Bundle.Empty.1")?;
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
        fixture_context_ref,
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
    // test_case.add_database_print_step()?; // disabled due to client-side fetch issue

    // Export the fixture’s transient pool into the test case’s session state.
    test_case.load_test_session_state(fixture_context_ref);

    Ok(test_case)
}

/// Declared relationship happy path (no type graph).
#[fixture]
pub async fn load_holons_declared_links_fixture() -> Result<DancesTestCase, HolonError> {
    let title = "Loader Declared Relationship Fixture".to_string();
    let desc = format!(
        "Declared relationship: ({})-[{}]->({})",
        BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY
    );
    let mut test_case = DancesTestCase::new(title, desc);

    let fixture_context_arc = init_fixture_context().await;
    let ctx: &dyn HolonsContextBehavior = &*fixture_context_arc;

    test_case.add_ensure_database_count_step(MapInteger(1))?;

    let (bundle, node_count, links_created) = build_declared_links_bundle(
        ctx,
        "Bundle.DeclaredLink.1",
        BOOK_KEY,                    // LoaderHolon Key = "Book.TheHollowTree"
        PERSON_1_KEY,                // LoaderHolon Key = "Person.AMonk"
        BOOK_TO_PERSON_RELATIONSHIP, // relationship_name on LRR
    )?;

    test_case.add_load_holons_step(
        bundle,
        ResponseStatusCode::OK,
        MapInteger(node_count as i64),
        MapInteger(node_count as i64),
        MapInteger(links_created as i64), // expect 1
        MapInteger(0),
    )?;

    test_case.add_ensure_database_count_step(MapInteger(1 + node_count as i64));

    test_case.load_test_session_state(ctx);
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
/// This function constructs the *loader-side* graph:
/// - `HasRelationshipReference`: LoaderHolon → LoaderRelationshipReference
/// - `ReferenceSource`: LRR → LoaderHolonReference (source, `holon_key = source_instance_key`)
/// - `ReferenceTarget`: LRR → LoaderHolonReference(s) (targets, ordered; `holon_key = target_instance_key`)
///
/// The resolver uses:
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
    // ── 1) Create LRR + endpoint containers under a short-lived write lock ──
    let (
        mut relationship_reference,
        mut source_ref,
        mut target_refs_uninitialized, // Vec<TransientReference>
    ) = {
        let transient_service_handle = context.get_space_manager().get_transient_behavior_service();
        let mut transient_service = transient_service_handle.write().map_err(|_| {
            HolonError::FailedToBorrow("Transient service lock was poisoned".into())
        })?;

        // 1a) LRR container (Key is descriptive; not used for endpoint resolution)
        let relationship_reference_key = format!(
            "LoaderRelationshipReference.{}.{}",
            source_instance_key, relationship_name_str
        );
        let relationship_reference =
            transient_service.create_empty(MapString(relationship_reference_key))?;

        // 1b) Source LoaderHolonReference container
        let source_ref_key = format!("LoaderHolonReference.Source.{}", source_instance_key);
        let source_ref = transient_service.create_empty(MapString(source_ref_key))?;

        // 1c) Target LoaderHolonReference containers
        let mut target_refs: Vec<TransientReference> =
            Vec::with_capacity(target_instance_keys.len());
        for (index, target_key) in target_instance_keys.iter().enumerate() {
            let target_ref_key = format!("LoaderHolonReference.Target{}.{}", index + 1, target_key);
            let target_ref = transient_service.create_empty(MapString(target_ref_key))?;
            target_refs.push(target_ref);
        }

        (relationship_reference, source_ref, target_refs)
    }; // 🔑 write lock released here

    // ── 2) Set properties on created transients ───────────────────────────────

    // 2a) LRR required properties
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

    // 2b) Source ref: holon_key = source instance key
    source_ref.with_property_value(
        context,
        CorePropertyTypeName::HolonKey.as_property_name(),
        BaseValue::StringValue(MapString(source_instance_key.to_string())),
    )?;

    // 2c) Target refs: holon_key = matching target instance keys (ordered)
    let mut target_ref_hrefs: Vec<HolonReference> =
        Vec::with_capacity(target_refs_uninitialized.len());
    for (mut target_ref, target_key) in
        target_refs_uninitialized.into_iter().zip(target_instance_keys.iter())
    {
        target_ref.with_property_value(
            context,
            CorePropertyTypeName::HolonKey.as_property_name(),
            BaseValue::StringValue(MapString((*target_key).to_string())),
        )?;
        target_ref_hrefs.push(HolonReference::Transient(target_ref));
    }

    // ── 3) Wire relationships on the loader graph ─────────────────────────────

    // 3a) LoaderHolon → HasRelationshipReference → LRR
    source_loader_holon.add_related_holons(
        context,
        CoreRelationshipTypeName::HasRelationshipReference.as_relationship_name().clone(),
        vec![HolonReference::Transient(relationship_reference.clone())],
    )?;

    // 3b) LRR → ReferenceSource → source_ref
    relationship_reference.add_related_holons(
        context,
        CoreRelationshipTypeName::ReferenceSource.as_relationship_name().clone(),
        vec![HolonReference::Transient(source_ref)],
    )?;

    // 3c) LRR → ReferenceTarget → target_refs (ordered)
    relationship_reference.add_related_holons(
        context,
        CoreRelationshipTypeName::ReferenceTarget.as_relationship_name().clone(),
        target_ref_hrefs,
    )?;

    Ok(relationship_reference)
}
