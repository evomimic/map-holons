// crates/holons_loader/src/loader_holon_mapper.rs
//
// Pass 1 mapper for the Holon Loader:
//   - For each incoming LoaderHolon (container), build a *target* TransientHolon with
//     *properties only* (no relationships), and stage it.
//   - Collect all LoaderRelationshipReference holons attached to each LoaderHolon
//     (via HAS_LOADER_RELATIONSHIP_REFERENCE) into a queue for Pass 2.
//
// This module intentionally avoids any relationship writes or type application;
// those are handled by the resolver in Pass 2.

use tracing::debug;

use holons_prelude::prelude::CorePropertyTypeName::Key;
use holons_prelude::prelude::CoreRelationshipTypeName::{BundleMembers, HasRelationshipReference};
use holons_prelude::prelude::*;

/// The mapper's output: staged nodes and queued edge-descriptors, owned
/// for the duration of the load call.
#[derive(Debug, Default)]
pub struct MapperOutput {
    /// Detached `LoaderRelationshipReference` transients queued for Pass-2 resolution.
    pub queued_relationship_references: Vec<TransientReference>,
    /// Non-fatal errors encountered during Pass-1 (e.g., missing key).
    pub errors: Vec<HolonError>,
    /// Number of successfully staged holons.
    pub staged_count: i64,
    // Optional: all staged holons (including keyless) for accurate staging stats.
    // pub staged_all: Vec<StagedReference>,  // (currently not needed)
}

/// LoaderHolonMapper: builds *target* node transients from LoaderHolons and stages them.
///
/// Notes:
/// - We deliberately do **not** set `DescribedBy` or any relationships here.
/// - We stage exactly one holon per LoaderHolon (relationships handled in Pass-2).
pub struct LoaderHolonMapper;

impl LoaderHolonMapper {
    /// Map the incoming bundle into staged holons & a queue of relationship refs.
    /// - Reads LoaderHolon members from the bundle
    /// - Stages properties-only holons; fills key_index
    /// - Collects LoaderRelationshipReference transients into queued_rel_refs
    pub fn map_bundle(
        context: &dyn HolonsContextBehavior,
        bundle: TransientReference,
    ) -> Result<MapperOutput, HolonError> {
        let mut out = MapperOutput::default();

        // Extract loader holons (bundle --BUNDLE_MEMBERS--> LoaderHolon*)
        // `related_holons` returns an Arc<RwLock<HolonCollection>>; lock it for read,
        // then clone the Vec so we can iterate without holding the lock.
        let loader_holon_members = {
            let loader_holon_collection_handle =
                bundle.related_holons(context, BundleMembers.as_relationship_name())?;

            let loader_holon_collection_guard =
                loader_holon_collection_handle.read().map_err(|_| {
                    HolonError::FailedToBorrow("HolonCollection read lock poisoned".into())
                })?;

            loader_holon_collection_guard.get_members().clone()
            // guard dropped here; lock released before we enter the loop
        };

        // Iterate through LoaderHolon members and stage target holons.
        for (index, loader_reference) in loader_holon_members.iter().enumerate() {
            debug!("Pass1: staging target from LoaderHolon #{}", index);

            match Self::build_target_staged(context, loader_reference) {
                Ok((_staged_reference, _loader_key)) => {
                    // Nursery will index by key; we only track counts and queue refs.
                    out.staged_count += 1;

                    // Queue relationship references only for successfully staged holons.
                    match Self::collect_loader_rel_refs(context, loader_reference) {
                        Ok(relationship_refs) => {
                            out.queued_relationship_references.extend(relationship_refs)
                        }
                        Err(err) => out.errors.push(err),
                    }
                }
                Err(err) => out.errors.push(err),
            }
        }

        Ok(out)
    }

    /// Build and immediately stage a *properties-only* target holon from a LoaderHolon.
    ///
    /// RETURNS:
    ///   (staged_target, loader_key)
    ///
    /// Invariants:
    /// - `key` MUST be present on the LoaderHolon (we currently do not support keyless).
    pub fn build_target_staged(
        context: &dyn HolonsContextBehavior,
        loader: &HolonReference,
    ) -> Result<(StagedReference, MapString), HolonError> {
        // Produce a detached TransientReference so we can access raw properties
        let loader_transient = loader.clone_holon(context)?;
        loader_transient.is_accessible(context, AccessType::Read)?;

        // Read the LoaderHolon's current property map (owned snapshot)
        let properties: PropertyMap = loader_transient.get_raw_property_map(context)?;

        // Identify this loader in error messages using its TemporaryId (stable within this call).
        let loader_id = loader_transient.get_temporary_id();

        // Ensure key exists (but do NOT remove it—leave it to be staged).
        let key_prop: PropertyName = Key.as_property_name();
        let key_value: PropertyValue = properties.get(&key_prop).cloned().ok_or_else(|| {
            HolonError::EmptyField(format!(
                "LoaderHolon.key missing (loader transient: {})",
                loader_id
            ))
        })?;

        // Convert key_value -> MapString for the return tuple (for logging/diagnostics if needed).
        let key = MapString((&key_value).into());

        // ── Create the empty transient under a short write lock, then immediately release it.
        let mut target_transient: TransientReference = {
            let transient_service_handle =
                context.get_space_manager().get_transient_behavior_service();
            let transient_service = transient_service_handle.write().map_err(|_| {
                HolonError::FailedToBorrow("TransientHolonBehavior lock poisoned".into())
            })?;
            debug!(
                "Pass-1: staging instance from LoaderHolon temp_id={:?}, key_prop_raw={:?}, create_empty_key=\"{}\"",
                loader_id,
                key_value,
                key.0,
            );
            transient_service.create_empty(key.clone())?
            // `transient_service` guard drops here — lock released before property writes.
        };

        // Apply each property explicitly (mutating the transient holon) — no service lock is held now.
        for (property_name, property_value) in properties.into_iter() {
            target_transient.with_property_value(context, &property_name, property_value)?;
        }

        // Stage it
        let staged = stage_new_holon(context, target_transient)?;

        Ok((staged, key))
    }

    /// Traverse from a LoaderHolon to all attached LoaderRelationshipReference holons and
    /// return them as **detached transients** for the resolver to consume.
    ///
    /// Relationship used: `HAS_LOADER_RELATIONSHIP_REFERENCE`.
    pub fn collect_loader_rel_refs(
        context: &dyn HolonsContextBehavior,
        loader: &HolonReference,
    ) -> Result<Vec<TransientReference>, HolonError> {
        let mut out: Vec<TransientReference> = Vec::new();

        // Direct traversal from LoaderHolon → LoaderRelationshipReference entries.
        let relationship_name = HasRelationshipReference.as_relationship_name();
        let collection_handle = loader.related_holons(context, &relationship_name)?;

        // Lock the collection for read, then clone members so we can release the lock early.
        let member_refs = {
            let collection_guard = collection_handle.read().map_err(|_| {
                HolonError::FailedToBorrow("HolonCollection read lock poisoned".into())
            })?;
            collection_guard.get_members().clone()
            // guard dropped here; lock released
        };

        // Work on **detached** copies so Pass-2 can resolve in any order/idempotently.
        for holon_reference in member_refs {
            let loader_relationship = holon_reference.clone_holon(context)?;
            out.push(loader_relationship);
        }

        Ok(out)
    }
}
