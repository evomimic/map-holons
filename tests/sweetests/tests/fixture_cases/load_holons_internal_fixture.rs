//! # Holon Loader Internal Test Fixtures (Incremental)
//!
//! This module provides an **incremental** rstest fixture that exercises the holon
//! loader's **controller behavior** in a single test case by appending multiple
//! bundles and assertions as steps. This keeps setup lean and mirrors real usage.
//!
//! Scope: loader-controller behavior only — empty bundles, nodes-only loads,
//! untyped-relationship rejection, duplicate-key failure, response metrics,
//! error holons, and `LoadCommitStatus`. Typed/inverse relationship **success**
//! coverage requires the full committed core schema (strict commit Pass 2
//! resolves every relationship against the source holon's effective schema
//! surface and materializes inverses), so it lives in the schema-backed
//! loader-client fixture (`load_book_person_inverse_schema_fixture`), not here.
//!
//! ## Fixture Progression (combined)
//!
//! 1. **Empty bundle** → `UnprocessableEntity`; database remains baseline (only Space holon)
//! 2. **Nodes-only bundle** (no relationships) → `OK`; LinksCreated = 0
//! 3. **Untyped declared relationship bundle** → pass-2 error; commit skipped
//! 4. **Multi-bundle duplicate-key set** (same LoaderHolon key in two files) → `UnprocessableEntity`; DB unchanged
//!
//! ### Why a single fixture?
//! - Enables incremental coverage growth by appending new steps (`add_load_holons_internal_step()`).
//! - Avoids repeated context setup; we export the same transient pool at the end.
//! - Keeps each step’s expectations explicit (status, staged/committed counts, links, errors).
//!
//! ## Key Implementation Notes
//!
//! - `context.mutation().new_holon(Some(key: String | MapString))` **sets the holon `Key` property automatically**,
//!   so we simply pass the *intended instance key string* when creating LoaderHolons.
//! - Arbitrary relationship names now require a resolvable type graph; an untyped relationship is
//!   intentionally covered as an error case.
//! - Pass-2 resolves `LoaderRelationshipReference` endpoints by `LoaderHolonReference.holon_key`
//!   (in-bundle first, then previously committed as your resolver specifies).
//!
//! Result: endpoint resolution uses consistent strings everywhere.

#![allow(unused_variables)]
#![allow(unused_mut)]

use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, ExpectedLoadStatus, TestCaseInit};
use rstest::*;
use std::sync::Arc;

use holons_test::harness::helpers::{BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY};

/// Simple wrapper for composing HolonLoadSets in tests:
/// one bundle + the filename it came from.
pub struct BundleWithFilename {
    pub bundle: TransientReference,
    pub filename: String,
}

impl BundleWithFilename {
    pub fn new(bundle: TransientReference, filename: &str) -> Self {
        Self { bundle, filename: filename.to_string() }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal bundle builders (kept private to this file)
// ─────────────────────────────────────────────────────────────────────────────

/// Build a HolonLoaderBundle **without** any BUNDLE_MEMBERS.
/// This intentionally exercises the “empty bundle” short-circuit path.
fn build_empty_bundle(
    context: &Arc<TransactionContext>,
    bundle_key: &str,
) -> Result<TransientReference, HolonError> {
    // New bundle transient holon with the provided key.
    let bundle = context.mutation().new_holon(Some(MapString(bundle_key.to_string())))?;
    Ok(bundle)
}

/// Build a HolonLoaderBundle with **N minimal LoaderHolons (nodes only)**.
/// Each member is created with its **instance key string**; no relationship references.
/// This exercises Pass-1 staging + commit with LinksCreated = 0.
fn build_nodes_only_bundle(
    context: &Arc<TransactionContext>,
    bundle_key: &str,
    instance_keys: &[&str],
) -> Result<(TransientReference, usize), HolonError> {
    // 1) Create the bundle container (Key = bundle_key).
    let mut bundle = context.mutation().new_holon(Some(MapString(bundle_key.to_string())))?;

    // 2) Create LoaderHolon containers with **consistent instance keys**.
    let mut members: Vec<HolonReference> = Vec::with_capacity(instance_keys.len());
    for key in instance_keys {
        let loader_holon = context.mutation().new_holon(Some(MapString((*key).to_string())))?;
        members.push(HolonReference::Transient(loader_holon));
    }

    // 3) Attach members via BUNDLE_MEMBERS.
    bundle.add_related_holons(
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
///   - Source LoaderHolon: `context.mutation().new_holon(source_instance_key)`
///   - Target LoaderHolon: `context.mutation().new_holon(target_instance_key)`
///   - LoaderHolonReference.holon_key values use the **same strings**
///
/// RETURNS: (bundle, expected_node_count, expected_links_created)
fn build_declared_links_bundle(
    context: &Arc<TransactionContext>,
    bundle_key: &str,
    source_instance_key: &str,
    target_instance_key: &str,
    declared_relationship_name: &str,
) -> Result<(TransientReference, usize, usize), HolonError> {
    // 1) Create the bundle container (Key = bundle_key).
    // LoaderHolons created with **instance key strings** (Key set automatically).
    let mut bundle = context.mutation().new_holon(Some(MapString(bundle_key.to_string())))?;
    let mut source_loader =
        context.mutation().new_holon(Some(MapString(source_instance_key.to_string())))?;
    let target_loader =
        context.mutation().new_holon(Some(MapString(target_instance_key.to_string())))?;

    // 2) Attach both as BundleMembers.
    bundle.add_related_holons(
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
        source_instance_key,
        &[target_instance_key],
    )?;

    Ok((bundle, 2, 1))
}

/// Build a HolonLoaderBundle that contains a single LoaderHolon with a
/// specific `Key` and `StartUtf8ByteOffset`.
///
/// Used to exercise multi-bundle duplicate-key detection + byte-offset
/// provenance in error holons.
///
/// RETURNS: (bundle, staged_node_count)
fn build_single_loader_with_offset_bundle(
    context: &Arc<TransactionContext>,
    bundle_key: &str,
    loader_key: &str,
    offset: i64,
) -> Result<(TransientReference, usize), HolonError> {
    // Bundle + single LoaderHolon (Key = loader_key)
    let mut bundle = context.mutation().new_holon(Some(MapString(bundle_key.to_string())))?;
    let mut loader = context.mutation().new_holon(Some(MapString(loader_key.to_string())))?;

    // Stamp the byte offset on the loader so the controller can enrich errors.
    set_start_offset(&mut loader, offset)?;

    // (HolonLoaderBundle)-[BUNDLE_MEMBERS]->(LoaderHolon)
    bundle.add_related_holons(
        CoreRelationshipTypeName::BundleMembers,
        vec![HolonReference::Transient(loader)],
    )?;

    Ok((bundle, 1))
}

// ─────────────────────────────────────────────────────────────────────────────
// Public fixture (returns a complete DancesTestCase)
// ─────────────────────────────────────────────────────────────────────────────

/// Combined loader-controller fixture:
///  1) Empty bundle → UnprocessableEntity; DB remains 1 (space holon)
///  2) Nodes-only bundle (3 nodes) → OK; LinksCreated=0; DB becomes 1 + 3
///  3) Untyped declared link bundle → pass-2 error; DB remains 1 + 3
///  4) Multi-bundle duplicate-key set (same LoaderHolon key in two files) → UnprocessableEntity; DB unchanged
///
/// Notes:
/// - The nodes-only keys are chosen to **avoid clashing** with the declared-link keys.
/// - Typed/inverse relationship success requires the full committed core schema and is
///   covered by the schema-backed loader-client fixture, not here.
/// - We export the fixture’s transient pool into the test case session_state state exactly once at the end.
#[fixture]
pub fn loader_incremental_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, mut fixture_bindings } =
        TestCaseInit::new(
            "Loader Incremental Fixture",
            "1) Ensure DB starts with only the Space holon,\n\
         2) Load a HolonLoadSet containing a single empty HolonLoaderBundle and assert the\n\
            loader short-circuits cleanly (no holons staged/committed, DB unchanged),\n\
         3) Load a nodes-only HolonLoadSet (Book/Person/Publisher LoaderHolons, no relationships)\n\
            and assert holons are staged + committed, LinksCreated = 0,\n\
         4) Load an untyped declared-relationship HolonLoadSet and assert the resolver rejects\n\
            it without committing staged holons,\n\
         5) Load a multi-bundle HolonLoadSet where two different bundles each contain a\n\
            LoaderHolon with the same Key but different filenames and byte offsets, and assert\n\
            the loader reports a duplicate-key error, skips commit (HolonsCommitted = 0),\n\
            leaves the DB unchanged, and surfaces per-file provenance via error holons.\n",
        );

    // A) Ensure DB starts with only the Space holon.
    test_case.add_ensure_database_count_step(MapInteger(1), None)?;

    // B) Empty bundle → expect UnprocessableEntity and no DB change.
    let empty_bundle = build_empty_bundle(&fixture_context, "Bundle.Empty.1")?;
    let empty_set = make_load_set_from_bundles(
        &fixture_context,
        "LoadSet.Empty.1",
        vec![BundleWithFilename::new(empty_bundle, "empty.json")],
    )?;
    test_case.add_load_holons_internal_step(
        empty_set,
        MapInteger(0), // holons_staged
        MapInteger(0), // holons_committed
        MapInteger(0), // links_created
        MapInteger(0), // errors_encountered (empty set short-circuit path)
        MapInteger(1), // total_bundles
        MapInteger(0), // total_loader_holons
        ExpectedLoadStatus::Skipped,
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1), None)?;

    // C) Nodes-only bundle → expect OK, N committed, 0 links created.
    let nodes_only_keys = &["Book.NodesOnly.1", "Person.NodesOnly.1", "Publisher.NodesOnly.1"];
    let (nodes_bundle, n_nodes) =
        build_nodes_only_bundle(&fixture_context, "Bundle.NodesOnly.1", nodes_only_keys)?;
    let nodes_set = make_load_set_from_bundles(
        &fixture_context,
        "LoadSet.NodesOnly.1",
        vec![BundleWithFilename::new(nodes_bundle, "nodes_only.json")],
    )?;
    test_case.add_load_holons_internal_step(
        nodes_set,
        MapInteger(n_nodes as i64), // holons_staged
        MapInteger(n_nodes as i64), // holons_committed
        MapInteger(0),              // links_created
        MapInteger(0),              // errors_encountered
        MapInteger(1),              // total_bundles
        MapInteger(n_nodes as i64), // total_loader_holons
        ExpectedLoadStatus::Complete,
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1 + n_nodes as i64), None)?;
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before declared-link load".to_string()),
    )?;

    // D) Untyped arbitrary relationships are now rejected because the guest resolver
    // requires a provable relationship type descriptor.
    let (declared_bundle, node_count, _links_created) = build_declared_links_bundle(
        &fixture_context,
        "Bundle.DeclaredLink.1",
        BOOK_KEY,
        PERSON_1_KEY,
        BOOK_TO_PERSON_RELATIONSHIP,
    )?;
    let declared_set = make_load_set_from_bundles(
        &fixture_context,
        "LoadSet.DeclaredLink.1",
        vec![BundleWithFilename::new(declared_bundle, "declared_link.json")],
    )?;
    test_case.add_load_holons_internal_step(
        declared_set,
        MapInteger(node_count as i64), // holons_staged
        MapInteger(0),                 // commit skipped
        MapInteger(0),                 // relationship could not be resolved
        MapInteger(1),                 // one pass-2 invalid-type error
        MapInteger(1),                 // total_bundles
        MapInteger(node_count as i64), // total_loader_holons
        ExpectedLoadStatus::Skipped,
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1 + n_nodes as i64), None)?;
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before duplicate-key load".to_string()),
    )?;

    // E) Multi-bundle duplicate-key failure:
    //
    // Two bundles, each containing a single LoaderHolon with the **same Key** but
    // different filenames and byte offsets. The controller should:
    //   - detect the duplicate loader_holon key across the HolonLoadSet,
    //   - emit a DuplicateError wrapped in ErrorWithContext,
    //   - enrich error holons with LoaderHolonKey + Filename + StartUtf8ByteOffset,
    //   - skip Pass 2 + commit (HolonsCommitted = 0),
    //   - leave the DB count unchanged.
    let dup_key = "MultiBundle.DuplicateKey.1";

    // Bundle G1: first occurrence (offset = 10)
    let (dup_bundle_1, dup_nodes_1) = build_single_loader_with_offset_bundle(
        &fixture_context,
        "Bundle.Multi.DuplicateKey.File1",
        dup_key,
        10,
    )?;

    // Bundle G2: second occurrence (offset = 200)
    let (dup_bundle_2, dup_nodes_2) = build_single_loader_with_offset_bundle(
        &fixture_context,
        "Bundle.Multi.DuplicateKey.File2",
        dup_key,
        200,
    )?;

    let dup_set = make_load_set_from_bundles(
        &fixture_context,
        "LoadSet.MultiBundle.DuplicateKey.1",
        vec![
            BundleWithFilename::new(dup_bundle_1, "multi_dup_file_1.json"),
            BundleWithFilename::new(dup_bundle_2, "multi_dup_file_2.json"),
        ],
    )?;

    let dup_total_nodes = (dup_nodes_1 + dup_nodes_2) as i64; // 1 + 1 = 2

    test_case.add_load_holons_internal_step(
        dup_set,
        MapInteger(dup_total_nodes), // holons_staged (Pass 1 still stages them)
        MapInteger(0),               // holons_committed (commit skipped)
        MapInteger(0),               // links_created
        MapInteger(1),               // errors_encountered (one DuplicateError)
        MapInteger(2),               // total_bundles
        MapInteger(dup_total_nodes), // total_loader_holons
        ExpectedLoadStatus::Skipped,
    )?;

    // DB must remain unchanged after duplicate-key failure.
    test_case.add_ensure_database_count_step(MapInteger(1 + n_nodes as i64), None)?;

    // Finalize
    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers for loader holon building
// ─────────────────────────────────────────────────────────────────────────────

/// Build and attach a `LoaderRelationshipReference` (LRR) to a given `LoaderHolon`,
/// wiring `ReferenceSource` and ordered `ReferenceTarget` endpoint containers,
/// and setting `relationship_name` in one place.
///
/// ### Behavior
/// - Creates a new LRR container and one `LoaderHolonReference` for the source,
///   plus one per target (in order).
/// - Sets:
///   - `relationship_name` (string)
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
/// - `source_instance_key`: key of the **instance** acting as relationship source
/// - `target_instance_keys`: keys of the **instance(s)** acting as relationship target(s)
///
/// ### Returns
/// - `Ok(TransientReference)` of the created LRR container
///
/// ### Notes
/// - `context.mutation().new_holon(Some(MapString(key)))` automatically sets the `Key` property; no
///   explicit `with_property_value(Key, ...)` needed.
/// - Endpoint resolution in Pass-2 relies on `LoaderHolonReference.holon_key`
///   values matching the instance keys you use for the corresponding LoaderHolons.
pub fn add_loader_relationship_reference(
    context: &Arc<TransactionContext>,
    source_loader_holon: &mut TransientReference,
    relationship_name_str: &str,
    source_instance_key: &str,
    target_instance_keys: &[&str],
) -> Result<TransientReference, HolonError> {
    // ── 1) Create LRR + endpoint containers under a short-lived write lock ──
    let (mut relationship_reference, mut source_ref, target_refs_uninitialized) = {
        // LRR container: key is descriptive; endpoint resolution does not use it.
        let relationship_reference_key = format!(
            "LoaderRelationshipReference.{}.{}",
            source_instance_key, relationship_name_str
        );
        let relationship_reference =
            context.mutation().new_holon(Some(MapString(relationship_reference_key)))?;

        // Source LoaderHolonReference container
        let source_ref_key = format!("LoaderHolonReference.Source.{}", source_instance_key);
        let source_ref = context.mutation().new_holon(Some(MapString(source_ref_key)))?;

        // Target LoaderHolonReference containers (ordered)
        let mut target_refs: Vec<TransientReference> =
            Vec::with_capacity(target_instance_keys.len());
        for (index, target_key) in target_instance_keys.iter().enumerate() {
            let target_ref_key = format!("LoaderHolonReference.Target{}.{}", index + 1, target_key);
            let target_ref = context.mutation().new_holon(Some(MapString(target_ref_key)))?;
            target_refs.push(target_ref);
        }

        (relationship_reference, source_ref, target_refs)
    };

    // ── 2) Set properties on created transients ───────────────────────────────

    // LRR required properties: relationship_name
    relationship_reference.with_property_value(
        CorePropertyTypeName::RelationshipName,
        BaseValue::StringValue(MapString(relationship_name_str.to_string())),
    )?;

    // Source endpoint: holon_key = source instance key
    source_ref.with_property_value(
        CorePropertyTypeName::HolonKey,
        BaseValue::StringValue(MapString(source_instance_key.to_string())),
    )?;

    // Target endpoints: holon_key = matching target instance keys (ordered)
    let mut target_ref_hrefs: Vec<HolonReference> =
        Vec::with_capacity(target_refs_uninitialized.len());
    for (mut target_ref, target_key) in
        target_refs_uninitialized.into_iter().zip(target_instance_keys.iter())
    {
        target_ref.with_property_value(
            CorePropertyTypeName::HolonKey,
            BaseValue::StringValue(MapString((*target_key).to_string())),
        )?;
        target_ref_hrefs.push(HolonReference::Transient(target_ref));
    }

    // ── 3) Wire relationships on the loader graph ─────────────────────────────
    // LoaderHolon → HasRelationshipReference → LRR
    source_loader_holon.add_related_holons(
        CoreRelationshipTypeName::HasRelationshipReference,
        vec![HolonReference::Transient(relationship_reference.clone())],
    )?;

    // LRR → ReferenceSource → source_ref
    relationship_reference.add_related_holons(
        CoreRelationshipTypeName::ReferenceSource,
        vec![HolonReference::Transient(source_ref)],
    )?;

    // LRR → ReferenceTarget → target_refs (ordered)
    relationship_reference
        .add_related_holons(CoreRelationshipTypeName::ReferenceTarget, target_ref_hrefs)?;

    Ok(relationship_reference)
}

/// Convenience: set the optional start byte offset on a LoaderHolon.
#[inline]
fn set_start_offset(loader: &mut TransientReference, offset: i64) -> Result<(), HolonError> {
    loader.with_property_value(
        CorePropertyTypeName::StartUtf8ByteOffset,
        BaseValue::IntegerValue(MapInteger(offset)),
    )?;
    Ok(())
}

// ───────────────────────────────────────────────────────────────────────────
// LoadSet helpers (ergonomic wrappers around existing bundle builders)
// ───────────────────────────────────────────────────────────────────────────

/// Attach the required `Filename` property to a HolonLoaderBundle.
fn set_bundle_filename(bundle: &mut TransientReference, filename: &str) -> Result<(), HolonError> {
    bundle.with_property_value(
        CorePropertyTypeName::Filename,
        BaseValue::StringValue(MapString(filename.to_string())),
    )?;
    Ok(())
}

/// Wrap one or more HolonLoaderBundle(s) into a HolonLoadSet and return the set.
///
/// `bundles` is a Vec of BundleWithFilename; for a single-bundle set,
/// just pass a Vec with one element.
pub fn make_load_set_from_bundles(
    context: &Arc<TransactionContext>,
    set_key: &str,
    bundles: Vec<BundleWithFilename>,
) -> Result<TransientReference, HolonError> {
    // 1) Create the set container
    let mut set_ref = context.mutation().new_holon(Some(MapString(set_key.to_string())))?;

    // 2) Stamp filenames on bundles and collect references
    let mut hrefs: Vec<HolonReference> = Vec::with_capacity(bundles.len());
    for spec in bundles {
        let mut bundle = spec.bundle;
        set_bundle_filename(&mut bundle, &spec.filename)?;
        hrefs.push(HolonReference::Transient(bundle));
    }

    // 3) (HolonLoadSet)-[CONTAINS]->(HolonLoaderBundle*)
    set_ref.add_related_holons(CoreRelationshipTypeName::Contains, hrefs)?;

    Ok(set_ref)
}
