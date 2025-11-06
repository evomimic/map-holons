//! # Holon Loader Test Fixtures (Incremental)
//!
//! This module provides an **incremental** rstest fixture that exercises the holon
//! loader’s two-pass workflow in a single test case by appending multiple bundles
//! and assertions as steps. This keeps setup lean and mirrors real usage.
//!
//! ## Fixture Progression (combined)
//!
//! 1. **Empty bundle** → `UnprocessableEntity`; database remains baseline (only Space holon)
//! 2. **Nodes-only bundle** (no relationships) → `OK`; LinksCreated = 0
//! 3. **Declared relationship bundle** (forward direction) → `OK`; LinksCreated = 1
//! 4. **Minimal micro-schema** (Book/Person + AUTHORED_BY + AUTHORS via InverseOf) → `OK`
//! 5. **Inverse LRR bundle** (Person AUTHORS Book) → `OK`; resolves to declared edge
//!
//! ### Why a single fixture?
//! - Enables incremental coverage growth by appending new steps (`add_load_holons_step()`).
//! - Avoids repeated context setup; we export the same transient pool at the end.
//! - Keeps each step’s expectations explicit (status, staged/committed counts, links, errors).
//!
//! ## Key Implementation Notes
//!
//! - `create_empty(key: String | MapString)` **sets the holon `Key` property automatically**,
//!   so we simply pass the *intended instance key string* when creating LoaderHolons.
//! - Tier-0 (declared-only) needs **no** type descriptors.
//! - For inverse mapping, we load a **tiny micro-schema** first (two HolonTypes + DeclaredRelationshipType + InverseRelationshipType).
//! - Pass-2 resolves `LoaderRelationshipReference` endpoints by `LoaderHolonReference.holon_key`
//!   (in-bundle first, then previously committed as your resolver specifies).
//!
//! Result: endpoint resolution uses consistent strings everywhere.

use crate::shared_test::test_data_types::{
    BOOK_DESCRIPTOR_KEY, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, BOOK_TO_PERSON_RELATIONSHIP_KEY,
    PERSON_1_KEY, PERSON_2_KEY, PERSON_DESCRIPTOR_KEY, PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY,
    PERSON_TO_BOOK_REL_INVERSE,
};
use crate::shared_test::{test_context::init_fixture_context, test_data_types::DancesTestCase};
use core_types::TypeKind;
use holons_prelude::prelude::*;
use rstest::*;

/// Declaredness of a `LoaderRelationshipReference` as represented by the
/// loader’s `IsDeclared` boolean property.
#[derive(Copy, Clone, Debug)]
pub enum LoaderRelationshipDeclaredness {
    /// Relationship is declared on the forward (canonical) side.
    Declared,
    /// Relationship is declared on the inverse side (resolver must map it).
    Inverse,
}

impl LoaderRelationshipDeclaredness {
    #[inline]
    fn as_map_boolean(self) -> MapBoolean {
        match self {
            LoaderRelationshipDeclaredness::Declared => MapBoolean(true),
            LoaderRelationshipDeclaredness::Inverse => MapBoolean(false),
        }
    }
}

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
    let transient_service = transient_service_handle
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
    add_loader_relationship_reference(
        context,
        &mut source_loader,
        declared_relationship_name,
        LoaderRelationshipDeclaredness::Declared,
        source_instance_key,
        &[target_instance_key],
    )?;

    Ok((bundle, 2, 1))
}

/// Build a HolonLoaderBundle to test **inverse LRR mapping**,
/// including inline micro-schema descriptors and two instances:
fn build_inverse_with_inline_schema_bundle(
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
    inverse_rel_name: &str,    // e.g., PERSON_TO_BOOK_REL_INVERSE ("Authors")
    person_instance_key: &str, // e.g., PERSON_2_KEY
    book_instance_key: &str,   // e.g., "Emerging World (Test Edition)"
) -> Result<(TransientReference, usize, usize), HolonError> {
    // 1) Create bundle + 6 loader holons (2 instances + 4 schema descriptors)
    let transient_service_handle = context.get_space_manager().get_transient_behavior_service();
    let transient_service = transient_service_handle
        .write()
        .map_err(|_| HolonError::FailedToBorrow("Transient service lock was poisoned".into()))?;

    let mut bundle = transient_service.create_empty(MapString(bundle_key.to_string()))?;

    // Instances
    let mut person_loader =
        transient_service.create_empty(MapString(person_instance_key.to_string()))?;
    let mut book_loader =
        transient_service.create_empty(MapString(book_instance_key.to_string()))?;

    // Schema descriptors (type + relationship types)
    let mut book_type_descriptor =
        transient_service.create_empty(MapString(BOOK_DESCRIPTOR_KEY.to_string()))?;
    let mut person_type_descriptor =
        transient_service.create_empty(MapString(PERSON_DESCRIPTOR_KEY.to_string()))?;
    let mut declared_rel_descriptor =
        transient_service.create_empty(MapString(BOOK_TO_PERSON_RELATIONSHIP_KEY.to_string()))?;
    let mut inverse_rel_descriptor = transient_service
        .create_empty(MapString(PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY.to_string()))?;
    drop(transient_service); // 🔓

    // Minimal names (helps diagnostics; not required for deref-by-key)
    for (transient_reference, name) in [
        (&mut book_type_descriptor, "Book"),
        (&mut person_type_descriptor, "Person"),
        (&mut declared_rel_descriptor, BOOK_TO_PERSON_RELATIONSHIP), // "AuthoredBy"
        (&mut inverse_rel_descriptor, PERSON_TO_BOOK_REL_INVERSE),   // "Authors"
    ] {
        transient_reference.with_property_value(
            context,
            CorePropertyTypeName::TypeName.as_property_name(),
            BaseValue::StringValue(MapString(name.to_string())),
        )?;
    }

    // Set TypeKind for relationship descriptors to enable
    // the loader's is_relationship_type_kind() check.
    for rel_descriptor in [&mut declared_rel_descriptor, &mut inverse_rel_descriptor] {
        rel_descriptor.with_property_value(
            context,
            CorePropertyTypeName::InstanceTypeKind.as_property_name(),
            BaseValue::StringValue(MapString(TypeKind::Relationship.to_string())),
        )?;
    }

    // 2) Add all six to the bundle as members
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers.as_relationship_name().clone(),
        vec![
            HolonReference::Transient(person_loader.clone()),
            HolonReference::Transient(book_loader.clone()),
            HolonReference::Transient(book_type_descriptor.clone()),
            HolonReference::Transient(person_type_descriptor.clone()),
            HolonReference::Transient(declared_rel_descriptor.clone()),
            HolonReference::Transient(inverse_rel_descriptor.clone()),
        ],
    )?;

    // 3) Wire schema links (DECLARED)
    add_loader_relationship_reference(
        context,
        &mut declared_rel_descriptor,
        CoreRelationshipTypeName::SourceType.as_relationship_name().0 .0.as_str(),
        LoaderRelationshipDeclaredness::Declared,
        BOOK_TO_PERSON_RELATIONSHIP_KEY,
        &[BOOK_DESCRIPTOR_KEY],
    )?;
    add_loader_relationship_reference(
        context,
        &mut declared_rel_descriptor,
        CoreRelationshipTypeName::TargetType.as_relationship_name().0 .0.as_str(),
        LoaderRelationshipDeclaredness::Declared,
        BOOK_TO_PERSON_RELATIONSHIP_KEY,
        &[PERSON_DESCRIPTOR_KEY],
    )?;
    add_loader_relationship_reference(
        context,
        &mut inverse_rel_descriptor,
        CoreRelationshipTypeName::InverseOf.as_relationship_name().0 .0.as_str(),
        LoaderRelationshipDeclaredness::Declared,
        PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY,
        &[BOOK_TO_PERSON_RELATIONSHIP_KEY],
    )?;

    // 4) Type the instances (DECLARED DescribedBy)
    let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
    add_loader_relationship_reference(
        context,
        &mut book_loader,
        described_by.0 .0.as_str(),
        LoaderRelationshipDeclaredness::Declared,
        book_instance_key,
        &[BOOK_DESCRIPTOR_KEY],
    )?;
    add_loader_relationship_reference(
        context,
        &mut person_loader,
        described_by.0 .0.as_str(),
        LoaderRelationshipDeclaredness::Declared,
        person_instance_key,
        &[PERSON_DESCRIPTOR_KEY],
    )?;

    // 5) Inverse LRR (Person --Authors--> Book) → maps to declared AuthoredBy(Book→Person)
    add_loader_relationship_reference(
        context,
        &mut person_loader,
        inverse_rel_name,
        LoaderRelationshipDeclaredness::Inverse,
        person_instance_key,
        &[book_instance_key],
    )?;

    // Staged nodes = 6; links created = 3 (schema) + 2 (DescribedBy) + 1 (declared) = 6
    Ok((bundle, 6, 6))
}

// ─────────────────────────────────────────────────────────────────────────────
// Public fixture (returns a complete DancesTestCase)
// ─────────────────────────────────────────────────────────────────────────────

/// Combined loader fixture:
///  1) Empty bundle → UnprocessableEntity; DB remains 1 (space holon)
///  2) Nodes-only bundle (3 nodes) → OK; LinksCreated=0; DB becomes 1 + 3
///  3) Declared link bundle (2 nodes, 1 link) → OK; DB becomes 1 + 3 + 2
///  4) Minimal micro-schema (4 nodes, 3 schema links) → OK; DB becomes 1 + 3 + 2 + 4
///  5) Inverse LRR bundle (1 node, maps to declared edge) → OK; DB becomes 1 + 3 + 2 + 4 + 1
///
/// Notes:
/// - The nodes-only keys are chosen to **avoid clashing** with the declared-link keys.
/// - The micro-schema enables inverse-name→declared-name mapping for Pass-2.
/// - Inverse bundle stages only a Person and references the existing Book by key.
/// - We export the fixture’s transient pool into the test case session state exactly once at the end.
#[fixture]
pub async fn loader_incremental_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Loader Incremental Fixture".to_string(),
        "Empty → nodes-only → declared link → micro-schema → inverse".to_string(),
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
        MapInteger(0), // ErrorCount (no Pass-1/2 errors; just short-circuit)
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1))?;

    // C) Nodes-only bundle → expect OK, N committed, 0 links created.
    let nodes_only_keys = &["Book.NodesOnly.1", "Person.NodesOnly.1", "Publisher.NodesOnly.1"];
    let (nodes_bundle, n_nodes) =
        build_nodes_only_bundle(fixture_context_ref, "Bundle.NodesOnly.1", nodes_only_keys)?;
    test_case.add_load_holons_step(
        nodes_bundle,
        ResponseStatusCode::OK,
        MapInteger(n_nodes as i64), // HolonsStaged
        MapInteger(n_nodes as i64), // HolonsCommitted
        MapInteger(0),              // LinksCreated
        MapInteger(0),              // ErrorCount
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1 + n_nodes as i64))?;

    // D) Declared relationship happy path (no type graph).
    let (declared_bundle, node_count, links_created) = build_declared_links_bundle(
        fixture_context_ref,
        "Bundle.DeclaredLink.1",
        BOOK_KEY,
        PERSON_1_KEY,
        BOOK_TO_PERSON_RELATIONSHIP, // e.g., "AUTHORED_BY"
    )?;
    test_case.add_load_holons_step(
        declared_bundle,
        ResponseStatusCode::OK,
        MapInteger(node_count as i64),    // 2
        MapInteger(node_count as i64),    // 2
        MapInteger(links_created as i64), // expect 1
        MapInteger(0),
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1 + n_nodes as i64 + node_count as i64))?;

    // E) Inverse LRR bundle: Person Authors Book → writes declared AuthoredBy(Book→Person).
    // Use a distinct book key to avoid colliding with the earlier Book instance.
    let inverse_book_key = "Emerging World (Test Edition)";

    let (inverse_bundle, inv_nodes, inv_links) = build_inverse_with_inline_schema_bundle(
        fixture_context_ref,
        "Bundle.InverseLink.1",
        PERSON_TO_BOOK_REL_INVERSE, // "Authors"
        PERSON_2_KEY,               // stage a new Person (2)
        inverse_book_key,           // stage a new Book with a distinct key
    )?;
    test_case.add_load_holons_step(
        inverse_bundle,
        ResponseStatusCode::OK,
        MapInteger(inv_nodes as i64), // 2
        MapInteger(inv_nodes as i64), // 2
        MapInteger(inv_links as i64), // 1 (declared edge written)
        MapInteger(0),
    )?;

    // Final DB count:
    // 1 (space) + n_nodes (3) + node_count (2) + schema_nodes (4) + inv_nodes (2) = 12
    test_case
        .add_ensure_database_count_step(MapInteger(1 + n_nodes as i64 + node_count as i64 + 6))?;

    // Export the fixture’s transient pool into the test case’s session state.
    test_case.load_test_session_state(fixture_context_ref);

    Ok(test_case)
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers for loader holon building
// ─────────────────────────────────────────────────────────────────────────────

/// Build and attach a `LoaderRelationshipReference` (LRR) to a given `LoaderHolon`,
/// wiring `ReferenceSource` and ordered `ReferenceTarget` endpoint containers,
/// and setting `relationship_name` plus `is_declared` in one place.
///
/// This replaces `add_declared_relationship_reference` and
/// `add_inverse_relationship_reference`.
///
/// ### Behavior
/// - Creates a new LRR container and one `LoaderHolonReference` for the source,
///   plus one per target (in order).
/// - Sets:
///   - `relationship_name` (string)
///   - `is_declared` (`true` for declared, `false` for inverse)
///   - Each endpoint’s `holon_key` to the provided instance keys
/// - Wires:
///   - `HasRelationshipReference` from the `source_loader_holon` to the LRR
///   - `ReferenceSource` from the LRR to the source ref
///   - `ReferenceTarget` from the LRR to each target ref (preserving order)
///
/// ### Parameters
/// - `context`: Holons context used for creation and wiring
/// - `source_loader_holon`: the **LoaderHolon** that owns this LRR
/// - `relationship_name_str`: the loader-declared relationship name
/// - `declaredness`: whether the LRR is declared or inverse
/// - `source_instance_key`: key of the **instance** acting as relationship source
/// - `target_instance_keys`: keys of the **instance(s)** acting as relationship target(s)
///
/// ### Returns
/// - `Ok(TransientReference)` of the created LRR container
///
/// ### Notes
/// - `create_empty(MapString(key))` automatically sets the `Key` property; no
///   explicit `with_property_value(Key, ..)` needed.
/// - Endpoint resolution in Pass-2 relies on `LoaderHolonReference.holon_key`
///   values matching the instance keys you use for the corresponding LoaderHolons.
pub fn add_loader_relationship_reference(
    context: &dyn HolonsContextBehavior,
    source_loader_holon: &mut TransientReference,
    relationship_name_str: &str,
    declaredness: LoaderRelationshipDeclaredness,
    source_instance_key: &str,
    target_instance_keys: &[&str],
) -> Result<TransientReference, HolonError> {
    // ── 1) Create LRR + endpoint containers under a short-lived write lock ──
    let (mut relationship_reference, mut source_ref, target_refs_uninitialized) = {
        let transient_service_handle = context.get_space_manager().get_transient_behavior_service();
        let transient_service = transient_service_handle.write().map_err(|_| {
            HolonError::FailedToBorrow("Transient service lock was poisoned".into())
        })?;

        // LRR container: key is descriptive; endpoint resolution does not use it.
        let relationship_reference_key = format!(
            "LoaderRelationshipReference.{}.{}",
            source_instance_key, relationship_name_str
        );
        let relationship_reference =
            transient_service.create_empty(MapString(relationship_reference_key))?;

        // Source LoaderHolonReference container
        let source_ref_key = format!("LoaderHolonReference.Source.{}", source_instance_key);
        let source_ref = transient_service.create_empty(MapString(source_ref_key))?;

        // Target LoaderHolonReference containers (ordered)
        let mut target_refs: Vec<TransientReference> =
            Vec::with_capacity(target_instance_keys.len());
        for (index, target_key) in target_instance_keys.iter().enumerate() {
            let target_ref_key = format!("LoaderHolonReference.Target{}.{}", index + 1, target_key);
            let target_ref = transient_service.create_empty(MapString(target_ref_key))?;
            target_refs.push(target_ref);
        }

        (relationship_reference, source_ref, target_refs)
    }; // 🔑 lock released here

    // ── 2) Set properties on created transients ───────────────────────────────

    // LRR required properties: relationship_name + is_declared
    relationship_reference.with_property_value(
        context,
        CorePropertyTypeName::RelationshipName.as_property_name(),
        BaseValue::StringValue(MapString(relationship_name_str.to_string())),
    )?;
    relationship_reference.with_property_value(
        context,
        CorePropertyTypeName::IsDeclared.as_property_name(),
        BaseValue::BooleanValue(declaredness.as_map_boolean()),
    )?;

    // Source endpoint: holon_key = source instance key
    source_ref.with_property_value(
        context,
        CorePropertyTypeName::HolonKey.as_property_name(),
        BaseValue::StringValue(MapString(source_instance_key.to_string())),
    )?;

    // Target endpoints: holon_key = matching target instance keys (ordered)
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
    // LoaderHolon → HasRelationshipReference → LRR
    source_loader_holon.add_related_holons(
        context,
        CoreRelationshipTypeName::HasRelationshipReference.as_relationship_name().clone(),
        vec![HolonReference::Transient(relationship_reference.clone())],
    )?;

    // LRR → ReferenceSource → source_ref
    relationship_reference.add_related_holons(
        context,
        CoreRelationshipTypeName::ReferenceSource.as_relationship_name().clone(),
        vec![HolonReference::Transient(source_ref)],
    )?;

    // LRR → ReferenceTarget → target_refs (ordered)
    relationship_reference.add_related_holons(
        context,
        CoreRelationshipTypeName::ReferenceTarget.as_relationship_name().clone(),
        target_ref_hrefs,
    )?;

    Ok(relationship_reference)
}
