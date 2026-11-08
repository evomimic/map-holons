//! Shared builders for loader inputs used by schema and attachment tests.

use holons_prelude::prelude::*;
use std::sync::Arc;

/// A loader bundle paired with its source filename.
pub struct BundleWithFilename {
    pub bundle: TransientReference,
    pub filename: String,
}

impl BundleWithFilename {
    pub fn new(bundle: TransientReference, filename: &str) -> Self {
        Self { bundle, filename: filename.to_string() }
    }
}

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
