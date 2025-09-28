// crates/holons_core/src/holon_loader/loader_holon_mapper.rs
//
// Pass 1 mapper for the Holon Loader:
//   - For each incoming LoaderHolon (container), build a *target* TransientHolon with
//     *properties only* (no relationships), and stage it.
//   - Record a fast (key → staged holon) mapping for Pass 2 resolution.
//   - Collect all LoaderRelationshipReference holons attached to each LoaderHolon
//     (via HAS_LOADER_RELATIONSHIP_REFERENCE) into a queue for Pass 2.
//
// This module intentionally avoids any relationship writes or type application;
// those are handled by the resolver in Pass 2.

use std::collections::HashMap;
use tracing::{debug, instrument};

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, PropertyMap, PropertyValue, PropertyName};

use crate::{
    // Re-exported in holons_core::lib
    HolonReference, HolonsContextBehavior, ReadableHolon, StagedReference,
    // High-level staging façade (re-exported from reference_layer::holon_operations_api)
    stage_new_holon_api,
};
use crate::reference_layer::{TransientReference, WritableHolon};
use crate::core_shared_objects::holon::state::AccessType;
use crate::core_shared_objects::HolonBehavior;
use crate::core_shared_objects::holon::HolonCloneModel;

use super::names as N;

/// The mapper's output: staged nodes and queued edge-descriptors, owned
/// for the duration of the load call.
#[derive(Debug, Default)]
pub struct MapperOutput {
    /// Fast key → staged holon for same-bundle resolution in Pass-2.
    pub key_index: HashMap<MapString, StagedReference>,
    /// Detached `LoaderRelationshipReference` transients queued for Pass-2 resolution.
    pub queued_rel_refs: Vec<TransientReference>,
    /// Non-fatal errors encountered during Pass-1 (e.g., missing key).
    pub errors: Vec<HolonError>,
    // pub staged_all: Vec<StagedReference>,
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
        let loader_holon_refs = bundle.related_holons(context, N::rel(N::REL_BUNDLE_MEMBERS))?;

        for (i, loader_ref) in loader_holon_refs.get_members().iter().enumerate() {
            debug!("Pass1: staging target from LoaderHolon #{}", i);

            match Self::build_target_staged(context, loader_ref) {
                Ok((staged, key)) => {
                    out.key_index.insert(key, staged.clone());
                    // queue rel refs only for successfully staged holons
                    match Self::collect_loader_rel_refs(context, loader_ref) {
                        Ok(rel_refs) => out.queued_rel_refs.extend(rel_refs),
                        Err(e) => out.errors.push(e),
                    }
                }
                Err(e) => out.errors.push(e),
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
    /// - Reserved props (`key`, `type`) are stripped prior to staging.
    #[instrument(level = "debug", skip_all)]
    pub fn build_target_staged(
        context: &dyn HolonsContextBehavior,
        loader: &HolonReference,
    ) -> Result<(StagedReference, MapString), HolonError> {
        // Produce a detached TransientReference so we can access raw properties
        let loader_transient = loader.clone_holon(context)?;

        loader_transient.is_accessible(context, AccessType::Read)?;

        // Read the LoaderHolon's current property map (owned snapshot)
        let mut props: PropertyMap = loader_transient.get_raw_property_map(context)?;

        // Identify this loader in error messages using its TemporaryId (stable within this call).
        let loader_id = loader_transient.get_temporary_id(); // cheap cloneable id

        // Pull key or error with loader id
        let key_prop: PropertyName = N::prop(N::PROP_KEY);
        let key_val: PropertyValue = props.remove(&key_prop).ok_or_else(|| {
            HolonError::EmptyField(format!(
                "LoaderHolon.key for loader transient: {},",
                loader_id
            ))
        })?;

        let key = MapString((&key_val).into());

        // Strip any other reserved loader properties (if present)
        let _ = props.remove(&N::prop(N::PROP_TYPE));

        // Build a properties-only clone model (no relationships, no predecessor).
        let clone_model = HolonCloneModel::new(MapInteger(0), None, props, None);

        // Mint a fresh transient and stage it.
        let transient_behavior_service = context.get_space_manager().get_transient_behavior_service();
        let transient_behavior = transient_behavior_service.borrow();
        let target_transient: TransientReference =
            transient_behavior.new_from_clone_model(clone_model)?;
        let staged = stage_new_holon_api(context, target_transient)?;

        Ok((staged, key))
    }

    /// Traverse from a LoaderHolon to all attached LoaderRelationshipReference holons and
    /// return them as **detached transients** for the resolver to consume.
    ///
    /// Relationship used: `HAS_LOADER_RELATIONSHIP_REFERENCE`.
    #[instrument(level = "debug", skip_all)]
    pub fn collect_loader_rel_refs(
        context: &dyn HolonsContextBehavior,
        loader: &HolonReference,
    ) -> Result<Vec<TransientReference>, HolonError> {
        let mut out: Vec<TransientReference> = Vec::new();

        // Direct traversal from LoaderHolon → LoaderRelationshipReference entries.
        let rel_name = N::rel(N::REL_HAS_REL_REF);
        let coll = loader.related_holons(context, &rel_name)?;

        coll.is_accessible(AccessType::Read)?;
        let members = coll.get_members();

        for h_ref in members {
            // Work on detached copies so Pass-2 can resolve in any order/idempotently.
            let loader_rel = h_ref.clone_holon(context)?;
            out.push(loader_rel);
        }

        Ok(out)
    }
}
