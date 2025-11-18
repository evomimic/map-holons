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
//! 6. **Multi-bundle set** (Book in one bundle, Person+link in another) → `OK`; cross-bundle resolution
//! 7. **Multi-bundle duplicate-key set** (same LoaderHolon key in two files) → `UnprocessableEntity`; DB unchanged
//!
//! ### Why a single fixture?
//! - Enables incremental coverage growth by appending new steps (`add_load_holons_step()`).
//! - Avoids repeated context setup; we export the same transient pool at the end.
//! - Keeps each step’s expectations explicit (status, staged/committed counts, links, errors).
//!
//! ## Key Implementation Notes
//!
//! - `new_holon(context, Some(key: String | MapString))` **sets the holon `Key` property automatically**,
//!   so we simply pass the *intended instance key string* when creating LoaderHolons.
//! - Tier-0 (declared-only) needs **no** type descriptors.
//! - For inverse mapping, we load a **tiny micro-schema** first (two HolonTypes + DeclaredRelationshipType + InverseRelationshipType).
//! - Pass-2 resolves `LoaderRelationshipReference` endpoints by `LoaderHolonReference.holon_key`
//!   (in-bundle first, then previously committed as your resolver specifies).
//!
//! Result: endpoint resolution uses consistent strings everywhere.

use core_types::TypeKind;
use holons_prelude::prelude::*;
use holons_test::DancesTestCase;
use rstest::*;

use crate::helpers::{BOOK_DESCRIPTOR_KEY, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, BOOK_TO_PERSON_RELATIONSHIP_KEY, PERSON_1_KEY, PERSON_2_KEY, PERSON_DESCRIPTOR_KEY, PERSON_TO_BOOK_REL_INVERSE, PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY, init_fixture_context};

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
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
) -> Result<TransientReference, HolonError> {
    // New bundle transient holon with the provided key.
    let bundle = new_holon(context, Some(MapString(bundle_key.to_string())))?;
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
    // 1) Create the bundle container (Key = bundle_key).
    let mut bundle = new_holon(context, Some(MapString(bundle_key.to_string())))?;

    // 2) Create LoaderHolon containers with **consistent instance keys**.
    let mut members: Vec<HolonReference> = Vec::with_capacity(instance_keys.len());
    for key in instance_keys {
        let loader_holon = new_holon(context, Some(MapString((*key).to_string())))?;
        members.push(HolonReference::Transient(loader_holon));
    }

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
///   - Source LoaderHolon: `new_holon(context, source_instance_key)`
///   - Target LoaderHolon: `new_holon(context, target_instance_key)`
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
    // LoaderHolons created with **instance key strings** (Key set automatically).
    let mut bundle = new_holon(context, Some(MapString(bundle_key.to_string())))?;
    let mut source_loader = new_holon(context, Some(MapString(source_instance_key.to_string())))?;
    let target_loader = new_holon(context, Some(MapString(target_instance_key.to_string())))?;

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

/// Build a HolonLoaderBundle that contains:
///   - One LoaderHolon representing the source instance
///   - One **declared** LoaderRelationshipReference whose target key may point
///     to a LoaderHolon in another bundle (or a previously saved holon).
///
/// This is used to exercise **cross-bundle** relationship resolution within
/// a single HolonLoadSet.
///
/// RETURNS: (bundle, expected_node_count, expected_links_created)
fn build_cross_bundle_declared_link_bundle(
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
    source_instance_key: &str,
    target_instance_key: &str,
    declared_relationship_name: &str,
) -> Result<(TransientReference, usize, usize), HolonError> {
    // Bundle + source LoaderHolon (target lives in a different bundle or is pre-existing)
    let mut bundle = new_holon(context, Some(MapString(bundle_key.to_string())))?;
    let mut source_loader = new_holon(context, Some(MapString(source_instance_key.to_string())))?;

    // Add source as BundleMember.
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers,
        vec![HolonReference::Transient(source_loader.clone())],
    )?;

    // Declared LRR: source_instance_key → target_instance_key
    // Target key may be in another bundle, but Pass-1 stages all loader holons
    // before Pass-2 resolution, so cross-bundle lookup is supported.
    add_loader_relationship_reference(
        context,
        &mut source_loader,
        declared_relationship_name,
        LoaderRelationshipDeclaredness::Declared,
        source_instance_key,
        &[target_instance_key],
    )?;

    // Nodes staged from this bundle = 1 (source instance); links_created = 1
    Ok((bundle, 1, 1))
}

/// Build a HolonLoaderBundle that contains a single LoaderHolon with a
/// specific `Key` and `StartUtf8ByteOffset`.
///
/// Used to exercise multi-bundle duplicate-key detection + byte-offset
/// provenance in error holons.
///
/// RETURNS: (bundle, staged_node_count)
fn build_single_loader_with_offset_bundle(
    context: &dyn HolonsContextBehavior,
    bundle_key: &str,
    loader_key: &str,
    offset: i64,
) -> Result<(TransientReference, usize), HolonError> {
    // Bundle + single LoaderHolon (Key = loader_key)
    let mut bundle = new_holon(context, Some(MapString(bundle_key.to_string())))?;
    let mut loader = new_holon(context, Some(MapString(loader_key.to_string())))?;

    // Stamp the byte offset on the loader so the controller can enrich errors.
    set_start_offset(context, &mut loader, offset)?;

    // (HolonLoaderBundle)-[BUNDLE_MEMBERS]->(LoaderHolon)
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers,
        vec![HolonReference::Transient(loader)],
    )?;

    Ok((bundle, 1))
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

    let mut bundle = new_holon(context, Some(MapString(bundle_key.to_string())))?;

    // Instances
    let mut person_loader = new_holon(context, Some(MapString(person_instance_key.to_string())))?;
    let mut book_loader = new_holon(context, Some(MapString(book_instance_key.to_string())))?;

    // Schema descriptors (type + relationship types)
    let mut book_type_descriptor =
        new_holon(context, Some(MapString(BOOK_DESCRIPTOR_KEY.to_string())))?;
    let mut person_type_descriptor =
        new_holon(context, Some(MapString(PERSON_DESCRIPTOR_KEY.to_string())))?;
    let mut declared_rel_descriptor =
        new_holon(context, Some(MapString(BOOK_TO_PERSON_RELATIONSHIP_KEY.to_string())))?;
    let mut inverse_rel_descriptor =
        new_holon(context, Some(MapString(PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY.to_string())))?;

    // Minimal names (helps diagnostics; not required for deref-by-key)
    for (transient_reference, name) in [
        (&mut book_type_descriptor, "Book"),
        (&mut person_type_descriptor, "Person"),
        (&mut declared_rel_descriptor, BOOK_TO_PERSON_RELATIONSHIP), // "AuthoredBy"
        (&mut inverse_rel_descriptor, PERSON_TO_BOOK_REL_INVERSE),   // "Authors"
    ] {
        transient_reference.with_property_value(
            context,
            CorePropertyTypeName::TypeName,
            BaseValue::StringValue(MapString(name.to_string())),
        )?;
    }

    // Set TypeKind for relationship descriptors to enable
    // the loader's is_relationship_type_kind() check.
    for rel_descriptor in [&mut declared_rel_descriptor, &mut inverse_rel_descriptor] {
        rel_descriptor.with_property_value(
            context,
            CorePropertyTypeName::InstanceTypeKind,
            BaseValue::StringValue(MapString(TypeKind::Relationship.to_string())),
        )?;
    }

    // 2) Add all six to the bundle as members
    bundle.add_related_holons(
        context,
        CoreRelationshipTypeName::BundleMembers,
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
///  6) Multi-bundle set (Book in one bundle, Person+link in another) → OK; DB becomes 1 + 3 + 2 + 4 + 1 + 2
///  7) Multi-bundle duplicate-key set (same LoaderHolon key in two files) → UnprocessableEntity; DB unchanged
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
        "1) Ensure DB starts with only the Space holon,\n\
         2) Load a HolonLoadSet containing a single empty HolonLoaderBundle and assert the\n\
            loader short-circuits cleanly (no holons staged/committed, DB unchanged),\n\
         3) Load a nodes-only HolonLoadSet (Book/Person/Publisher LoaderHolons, no relationships)\n\
            and assert holons are staged + committed, LinksCreated = 0,\n\
         4) Load a declared-relationship HolonLoadSet (Book + Person in one bundle with a\n\
            declared AUTHORED_BY edge) and assert 2 holons committed and 1 link created,\n\
         5) Load a HolonLoadSet that includes a minimal inline micro-schema (Book/Person\n\
            HolonTypes + AuthoredBy/Authors RelationshipTypes) and exercise inverse-name\n\
            resolution so a Person AUTHORS Book edge is written as the declared AuthoredBy\n\
           (Book→Person) relationship,\n\
         6) Load a multi-bundle HolonLoadSet where the Book LoaderHolon lives in one bundle\n\
            and the Person + declared AuthoredBy edge live in another, and assert cross-bundle\n\
            endpoint resolution works (both holons committed, 1 link created),\n\
         7) Load a multi-bundle HolonLoadSet where two different bundles each contain a\n\
            LoaderHolon with the same Key but different filenames and byte offsets, and assert\n\
            the loader reports a duplicate-key error, skips commit (HolonsCommitted = 0),\n\
            leaves the DB unchanged, and surfaces per-file provenance via error holons.\n"
            .to_string(),
    );

    // Create a private fixture context with its own TransientHolonManager.
    let fixture_context_arc = init_fixture_context();
    let fixture_context_ref: &dyn HolonsContextBehavior = &*fixture_context_arc;

    // A) Ensure DB starts with only the Space holon.
    test_case.add_ensure_database_count_step(MapInteger(1))?;

    // B) Empty bundle → expect UnprocessableEntity and no DB change.
    let empty_bundle = build_empty_bundle(fixture_context_ref, "Bundle.Empty.1")?;
    let empty_set = make_load_set_from_bundles(
        fixture_context_ref,
        "LoadSet.Empty.1",
        vec![BundleWithFilename::new(empty_bundle, "empty.json")],
    )?;
    test_case.add_load_holons_step(
        empty_set,
        MapInteger(0), // holons_staged
        MapInteger(0), // holons_committed
        MapInteger(0), // links_created
        MapInteger(0), // errors_encountered (empty set short-circuit path)
        MapInteger(1), // total_bundles
        MapInteger(0), // total_loader_holons
    )?;
    test_case.add_ensure_database_count_step(MapInteger(1))?;

    // C) Nodes-only bundle → expect OK, N committed, 0 links created.
    let nodes_only_keys = &["Book.NodesOnly.1", "Person.NodesOnly.1", "Publisher.NodesOnly.1"];
    let (nodes_bundle, n_nodes) =
        build_nodes_only_bundle(fixture_context_ref, "Bundle.NodesOnly.1", nodes_only_keys)?;
    let nodes_set = make_load_set_from_bundles(
        fixture_context_ref,
        "LoadSet.NodesOnly.1",
        vec![BundleWithFilename::new(nodes_bundle, "nodes_only.json")],
    )?;
    test_case.add_load_holons_step(
        nodes_set,
        MapInteger(n_nodes as i64), // holons_staged
        MapInteger(n_nodes as i64), // holons_committed
        MapInteger(0),              // links_created
        MapInteger(0),              // errors_encountered
        MapInteger(1),              // total_bundles
        MapInteger(n_nodes as i64), // total_loader_holons
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
    let declared_set = make_load_set_from_bundles(
        fixture_context_ref,
        "LoadSet.DeclaredLink.1",
        vec![BundleWithFilename::new(declared_bundle, "declared_link.json")],
    )?;
    test_case.add_load_holons_step(
        declared_set,
        MapInteger(node_count as i64),    // 2
        MapInteger(node_count as i64),    // 2
        MapInteger(links_created as i64), // expect 1
        MapInteger(0),
        MapInteger(1),                 // total_bundles
        MapInteger(node_count as i64), // total_loader_holons
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
    let inverse_set = make_load_set_from_bundles(
        fixture_context_ref,
        "LoadSet.InverseLink.1",
        vec![BundleWithFilename::new(inverse_bundle, "inverse_link.json")],
    )?;
    test_case.add_load_holons_step(
        inverse_set,
        MapInteger(inv_nodes as i64), // 2
        MapInteger(inv_nodes as i64), // 2
        MapInteger(inv_links as i64), // 1 (declared edge written)
        MapInteger(0),
        MapInteger(1),                // total_bundles
        MapInteger(inv_nodes as i64), // total_loader_holons
    )?;

    // DB after inverse step:
    // 1 (space) + n_nodes (3) + node_count (2) + inv_nodes (6) = 12
    let post_inverse_db_count = 1 + n_nodes as i64 + node_count as i64 + inv_nodes as i64;
    test_case.add_ensure_database_count_step(MapInteger(post_inverse_db_count))?;

    // F) Multi-bundle happy path:
    //
    // Bundle F1: Book node only (no relationships)
    // Bundle F2: Person node + declared AuthoredBy(Book→Person) LRR
    //            where the Book key lives in F1.
    //
    // This exercises cross-bundle resolution within a single HolonLoadSet.
    let multi_book_key = "MultiBundle.Book.1";
    let multi_person_key = "MultiBundle.Person.1";

    // Bundle F1: Book node only
    let (bundle_f1, f1_nodes) =
        build_nodes_only_bundle(fixture_context_ref, "Bundle.Multi.BookOnly", &[multi_book_key])?;

    // Bundle F2: Person node + declared AuthoredBy link pointing to Book in F1
    let (bundle_f2, f2_nodes, f_links) = build_cross_bundle_declared_link_bundle(
        fixture_context_ref,
        "Bundle.Multi.PersonWithLink",
        multi_person_key,            // source (Person)
        multi_book_key,              // target (Book in Bundle F1)
        BOOK_TO_PERSON_RELATIONSHIP, // "AuthoredBy"
    )?;

    let multi_set = make_load_set_from_bundles(
        fixture_context_ref,
        "LoadSet.MultiBundle.1",
        vec![
            BundleWithFilename::new(bundle_f1, "multi_book.json"),
            BundleWithFilename::new(bundle_f2, "multi_person_with_link.json"),
        ],
    )?;

    let multi_bundle_nodes_total = (f1_nodes + f2_nodes) as i64; // 1 Book + 1 Person = 2

    test_case.add_load_holons_step(
        multi_set,
        MapInteger(multi_bundle_nodes_total), // holons_staged
        MapInteger(multi_bundle_nodes_total), // holons_committed
        MapInteger(f_links as i64),           // links_created = 1
        MapInteger(0),                        // errors_encountered
        MapInteger(2),                        // total_bundles
        MapInteger(multi_bundle_nodes_total), // total_loader_holons
    )?;

    // Final DB count after multi-bundle happy path:
    // 12 (post-inverse) + 2 (multi-bundle Book + Person) = 14
    let post_multi_db_count = post_inverse_db_count + multi_bundle_nodes_total;
    test_case.add_ensure_database_count_step(MapInteger(post_multi_db_count))?;

    // G) Multi-bundle duplicate-key failure:
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
        fixture_context_ref,
        "Bundle.Multi.DuplicateKey.File1",
        dup_key,
        10,
    )?;

    // Bundle G2: second occurrence (offset = 200)
    let (dup_bundle_2, dup_nodes_2) = build_single_loader_with_offset_bundle(
        fixture_context_ref,
        "Bundle.Multi.DuplicateKey.File2",
        dup_key,
        200,
    )?;

    let dup_set = make_load_set_from_bundles(
        fixture_context_ref,
        "LoadSet.MultiBundle.DuplicateKey.1",
        vec![
            BundleWithFilename::new(dup_bundle_1, "multi_dup_file_1.json"),
            BundleWithFilename::new(dup_bundle_2, "multi_dup_file_2.json"),
        ],
    )?;

    let dup_total_nodes = (dup_nodes_1 + dup_nodes_2) as i64; // 1 + 1 = 2

    test_case.add_load_holons_step(
        dup_set,
        MapInteger(dup_total_nodes), // holons_staged (Pass 1 still stages them)
        MapInteger(0),               // holons_committed (commit skipped)
        MapInteger(0),               // links_created
        MapInteger(1),               // errors_encountered (one DuplicateError)
        MapInteger(2),               // total_bundles
        MapInteger(dup_total_nodes), // total_loader_holons
    )?;

    // DB must remain unchanged after duplicate-key failure.
    test_case.add_ensure_database_count_step(MapInteger(post_multi_db_count))?;

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
/// - `new_holon(context, Some(MapString(key)))` automatically sets the `Key` property; no
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
        // LRR container: key is descriptive; endpoint resolution does not use it.
        let relationship_reference_key = format!(
            "LoaderRelationshipReference.{}.{}",
            source_instance_key, relationship_name_str
        );
        let relationship_reference =
            new_holon(context, Some(MapString(relationship_reference_key)))?;

        // Source LoaderHolonReference container
        let source_ref_key = format!("LoaderHolonReference.Source.{}", source_instance_key);
        let source_ref = new_holon(context, Some(MapString(source_ref_key)))?;

        // Target LoaderHolonReference containers (ordered)
        let mut target_refs: Vec<TransientReference> =
            Vec::with_capacity(target_instance_keys.len());
        for (index, target_key) in target_instance_keys.iter().enumerate() {
            let target_ref_key = format!("LoaderHolonReference.Target{}.{}", index + 1, target_key);
            let target_ref = new_holon(context, Some(MapString(target_ref_key)))?;
            target_refs.push(target_ref);
        }

        (relationship_reference, source_ref, target_refs)
    };

    // ── 2) Set properties on created transients ───────────────────────────────

    // LRR required properties: relationship_name + is_declared
    relationship_reference.with_property_value(
        context,
        CorePropertyTypeName::RelationshipName,
        BaseValue::StringValue(MapString(relationship_name_str.to_string())),
    )?;
    relationship_reference.with_property_value(
        context,
        CorePropertyTypeName::IsDeclared,
        BaseValue::BooleanValue(declaredness.as_map_boolean()),
    )?;

    // Source endpoint: holon_key = source instance key
    source_ref.with_property_value(
        context,
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
            context,
            CorePropertyTypeName::HolonKey,
            BaseValue::StringValue(MapString((*target_key).to_string())),
        )?;
        target_ref_hrefs.push(HolonReference::Transient(target_ref));
    }

    // ── 3) Wire relationships on the loader graph ─────────────────────────────
    // LoaderHolon → HasRelationshipReference → LRR
    source_loader_holon.add_related_holons(
        context,
        CoreRelationshipTypeName::HasRelationshipReference,
        vec![HolonReference::Transient(relationship_reference.clone())],
    )?;

    // LRR → ReferenceSource → source_ref
    relationship_reference.add_related_holons(
        context,
        CoreRelationshipTypeName::ReferenceSource,
        vec![HolonReference::Transient(source_ref)],
    )?;

    // LRR → ReferenceTarget → target_refs (ordered)
    relationship_reference.add_related_holons(
        context,
        CoreRelationshipTypeName::ReferenceTarget,
        target_ref_hrefs,
    )?;

    Ok(relationship_reference)
}

/// Convenience: set the optional start byte offset on a LoaderHolon.
#[inline]
fn set_start_offset(
    context: &dyn HolonsContextBehavior,
    loader: &mut TransientReference,
    offset: i64,
) -> Result<(), HolonError> {
    loader.with_property_value(
        context,
        CorePropertyTypeName::StartUtf8ByteOffset,
        BaseValue::IntegerValue(MapInteger(offset)),
    )?;
    Ok(())
}

// ───────────────────────────────────────────────────────────────────────────
// LoadSet helpers (ergonomic wrappers around existing bundle builders)
// ───────────────────────────────────────────────────────────────────────────

/// Attach the required `Filename` property to a HolonLoaderBundle.
fn set_bundle_filename(
    context: &dyn HolonsContextBehavior,
    bundle: &mut TransientReference,
    filename: &str,
) -> Result<(), HolonError> {
    bundle.with_property_value(
        context,
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
    context: &dyn HolonsContextBehavior,
    set_key: &str,
    bundles: Vec<BundleWithFilename>,
) -> Result<TransientReference, HolonError> {
    // 1) Create the set container
    let mut set_ref = new_holon(context, Some(MapString(set_key.to_string())))?;

    // 2) Stamp filenames on bundles and collect references
    let mut hrefs: Vec<HolonReference> = Vec::with_capacity(bundles.len());
    for spec in bundles {
        let mut bundle = spec.bundle;
        set_bundle_filename(context, &mut bundle, &spec.filename)?;
        hrefs.push(HolonReference::Transient(bundle));
    }

    // 3) (HolonLoadSet)-[CONTAINS]->(HolonLoaderBundle*)
    set_ref.add_related_holons(context, CoreRelationshipTypeName::Contains, hrefs)?;

    Ok(set_ref)
}
