// shared_crates/holons_loader/src/loader_holon_mapper.rs
//
// Pass 1 mapper for the Holon Loader:
//   - For each incoming LoaderHolon (container), build a *target* TransientHolon with
//     *properties only* (no relationships), and stage it.
//   - Collect all LoaderRelationshipReference holons attached to each LoaderHolon
//     (via HAS_LOADER_RELATIONSHIP_REFERENCE) into a queue for Pass 2.
//
// This module intentionally avoids any relationship writes or type application;
// those are handled by the resolver in Pass 2.

use std::sync::Arc;
use tracing::{debug, warn};

use holons_prelude::prelude::CorePropertyTypeName::{Key, StartUtf8ByteOffset};
use holons_prelude::prelude::CoreRelationshipTypeName::{BundleMembers, HasRelationshipReference};
use holons_prelude::prelude::*;

use crate::errors::ErrorWithContext;

/// The mapper's output: staged nodes and queued edge-descriptors, owned
/// for the duration of the load call.
#[derive(Debug, Default)]
pub struct MapperOutput {
    /// Detached `LoaderRelationshipReference` transients queued for Pass-2 resolution.
    pub queued_relationship_references: Vec<TransientReference>,
    /// Non-fatal errors encountered during Pass-1 (e.g., missing key).
    pub errors: Vec<ErrorWithContext>,
    /// Number of successfully staged holons.
    pub staged_count: i64,
    /// Number of LoaderHolons present in a bundle (for diagnostics).
    pub loader_holon_count: i64,
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
    /// - Stages properties-only holons (filters loader-only props); fills counts
    /// - Collects LoaderRelationshipReference transients into `queued_relationship_references`
    pub fn map_bundle(
        context: &Arc<TransactionContext>,
        bundle: TransientReference,
    ) -> Result<MapperOutput, HolonError> {
        let mut output = MapperOutput::default();

        // Extract loader holons (bundle --BUNDLE_MEMBERS--> LoaderHolon*).
        //
        // Locking & safety:
        //   The bundle’s relationship map is immutable once parsing completes (loader phase has no writers).
        //   It is therefore safe to hold the read lock while iterating members to avoid cloning the collection.
        let collection_handle = bundle.related_holons(&BundleMembers)?;
        let guard = collection_handle
            .read()
            .map_err(|_| HolonError::FailedToBorrow("HolonCollection read lock poisoned".into()))?;
        let members = guard.get_members();

        let loader_holon_count = members.len() as i64;
        output.loader_holon_count = loader_holon_count;

        if members.is_empty() {
            warn!("LoaderHolonMapper.map_bundle: bundle has zero LoaderHolon members");
            return Ok(output); // empty output
        }

        for (index, loader_reference) in members.iter().enumerate() {
            debug!("Pass1: staging target from LoaderHolon #{}", index);

            match Self::build_target_staged(context, loader_reference) {
                Ok((_staged_reference, loader_key)) => {
                    // Nursery will index by key; we only track counts and queue refs.
                    output.staged_count += 1;

                    // Queue relationship references only for successfully staged holons.
                    match Self::collect_loader_rel_refs(loader_reference) {
                        Ok(relationship_refs) => {
                            output.queued_relationship_references.extend(relationship_refs)
                        }
                        Err(err) => output
                            .errors
                            .push(ErrorWithContext::new(err).with_loader_key(loader_key)),
                    }
                }
                Err(err) => {
                    // Missing key error
                    output.errors.push(ErrorWithContext::new(err));
                }
            }
        }

        Ok(output)
    }

    /// Build and immediately stage a *properties-only* target holon from a LoaderHolon.
    ///
    /// RETURNS:
    ///   (staged_target, loader_key)
    ///
    /// Invariants:
    /// - `key` MUST be present on the LoaderHolon (we currently do not support keyless).
    /// - Loader-only properties (e.g., `StartUtf8ByteOffset`) are **not** copied to the target.
    pub fn build_target_staged(
        context: &Arc<TransactionContext>,
        loader: &HolonReference,
    ) -> Result<(StagedReference, MapString), HolonError> {
        // Produce a detached TransientReference so we can access raw properties
        let loader_transient = loader.clone_holon()?;
        loader_transient.is_accessible(AccessType::Read)?;

        // Read the LoaderHolon's current property map (owned snapshot)
        let properties: PropertyMap = loader_transient.get_raw_property_map(context)?;

        // Identify this loader in error messages using its TemporaryId (stable within this call).
        let loader_id = loader_transient.temporary_id();

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
            let transient_behavior = context.get_transient_behavior_service();
            debug!(
                "Pass-1: staging instance from LoaderHolon temp_id={:?}, key_prop_raw={:?}, create_empty_key=\"{}\"",
                loader_id,
                key_value,
                key.0,
            );
            transient_behavior.create_empty(key.clone())?
        };

        // Apply each property explicitly (mutating the transient holon) — no service lock is held now.
        // Filter out loader-only properties (e.g., StartUtf8ByteOffset).
        let skip_loader_only_prop: PropertyName = StartUtf8ByteOffset.as_property_name();
        for (property_name, property_value) in properties.into_iter() {
            if property_name == skip_loader_only_prop {
                debug!(
                    "Pass-1: skipping loader-only property {:?} on LoaderHolon temp_id={:?}",
                    property_name, loader_id
                );
                continue;
            }
            target_transient.with_property_value(&property_name, property_value)?;
        }

        // Stage it
        let staged = context.mutation().stage_new_holon(target_transient)?;

        Ok((staged, key))
    }

    /// Traverse from a LoaderHolon to all attached LoaderRelationshipReference holons and
    /// return them as **detached transients** for the resolver to consume.
    ///
    /// Relationship used: `HAS_LOADER_RELATIONSHIP_REFERENCE`.
    pub fn collect_loader_rel_refs(
        loader: &HolonReference,
    ) -> Result<Vec<TransientReference>, HolonError> {
        // Direct traversal from LoaderHolon → LoaderRelationshipReference entries.
        let relationship_name = HasRelationshipReference;
        let collection_handle = loader.related_holons(&relationship_name)?;

        // Lock the collection for read and iterate members without cloning the entire collection.
        //
        // Safety:
        //   The relationship map is immutable during the loader phase (no writers),
        //   so it is safe to retain the read lock while collecting references.
        let guard = collection_handle
            .read()
            .map_err(|_| HolonError::FailedToBorrow("HolonCollection read lock poisoned".into()))?;
        let member_refs = guard.get_members();

        let mut output: Vec<TransientReference> = Vec::new();
        // Work on **detached** copies so Pass-2 can resolve in any order/idempotently.
        for holon_reference in member_refs {
            let loader_relationship = holon_reference.clone_holon()?;
            output.push(loader_relationship);
        }

        Ok(output)
    }
}
