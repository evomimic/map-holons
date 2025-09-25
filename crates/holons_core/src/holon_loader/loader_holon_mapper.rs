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

use tracing::{debug, instrument};

use base_types::{BaseValue, MapString};
use core_types::HolonError;
use core_types::PropertyMap;
use crate::{
    // Re-exported in holons_core::lib
    HolonReference, HolonsContextBehavior, ReadableHolon, StagedReference,
    // High-level staging façade (re-exported from reference_layer::holon_operations_api)
    stage_new_holon_api,
};
use crate::core_shared_objects::holon::state::AccessType;
use crate::core_shared_objects::HolonBehavior;
use crate::core_shared_objects::holon::TransientHolon;
use crate::reference_layer::ReadableHolonReferenceLayer;
use super::names as N;

/// The mapper's output: staged nodes and queued edge-descriptors.
#[derive(Debug, Default)]
pub struct MapperOutput {
    /// Pairs of (key, staged holon) for fast Pass-2 lookups.
    pub keyed_staged: Vec<(MapString, StagedReference)>,
    /// Detached `LoaderRelationshipReference` transients queued for Pass-2 resolution.
    pub queued_rel_refs: Vec<TransientHolon>,
}

/// LoaderHolonMapper: builds *target* node transients from LoaderHolons and stages them.
///
/// Notes:
/// - We deliberately do **not** set `DescribedBy` or any relationships here.
/// - We stage exactly one holon per LoaderHolon (plus any embedded targets are handled in Pass-2).
pub struct LoaderHolonMapper;

impl LoaderHolonMapper {
    /// Map & stage all LoaderHolons in one pass, collecting a key-index and the queued edge refs.
    pub fn map_and_stage(
        context: &dyn HolonsContextBehavior,
        loader_holons: &[HolonReference],
    ) -> Result<MapperOutput, HolonError> {
        let mut out = MapperOutput::default();

        for (i, loader_ref) in loader_holons.iter().enumerate() {
            debug!("Pass1: staging target from LoaderHolon #{i}");

            // Build and stage target holon
            let (staged, key_opt) = Self::build_target_staged(context, loader_ref)?;

            // Index by the loader holon's key if present
            if let Some(key) = key_opt {
                out.keyed_staged.push((key, staged.clone()));
            }

            // Collect loader relationship references
            let rel_refs = Self::collect_loader_rel_refs(context, loader_ref)?;
            out.queued_rel_refs.extend(rel_refs);
        }

        Ok(out)
    }


    /// Build and immediately stage a *properties-only* target holon from a LoaderHolon.
    ///
    /// RETURNS:
    ///   (staged_target, loader_key) where loader_key is *not* committed.
    ///
    /// IMPORTANT:
    /// - We copy *all instance properties except* the loader-only `key` and `type`.
    /// - `key` is derivable (per key rules) and must *not* be committed as a property.
    /// - `type` is converted to a DescribedBy relationship in Pass 2 (resolver), so it
    ///   must *not* be committed as a property here.
    pub fn build_target_staged(
        context: &dyn HolonsContextBehavior,
        loader: &HolonReference,
    ) -> Result<(StagedReference, Option<MapString>), HolonError> {
        // Read the LoaderHolon's current property map
        let holon_content = loader.essential_content(context)?;
        let mut property_map: PropertyMap = holon_content.property_map;

        // Capture the loader key (if any) before stripping
        let key_opt: Option<MapString> = property_map
            .get(&N::prop(N::PROP_KEY))
            .and_then(|opt| opt.as_ref())
            .and_then(|bv| match bv {
                BaseValue::StringValue(ms) => Some(ms.clone()),
                _ => None,
            });

        // Strip loader-only fields: `key`, `type`
        property_map.remove(&N::prop(N::PROP_KEY));
        // ToDo: deal with 'type'/DescribedBy relationship if needed
        //property_map.remove(&N::prop(N::PROP_TYPE));

        // Build a fresh transient and set the filtered property map
        let mut target = TransientHolon::new();
        target.update_property_map(property_map)?;

        // Stage it to obtain a writable staged reference
        let staged = stage_new_holon_api(context, target)?;
        Ok((staged, key_opt))
    }


    /// Traverse from a LoaderHolon to all attached LoaderRelationshipReference holons and
    /// return them as **detached transients** for the resolver to consume.
    ///
    /// Relationship used: `HAS_LOADER_RELATIONSHIP_REFERENCE`.
    #[instrument(level = "debug", skip_all)]
    pub fn collect_loader_rel_refs(
        context: &dyn HolonsContextBehavior,
        loader: &HolonReference,
    ) -> Result<Vec<TransientHolon>, HolonError> {
        let mut out: Vec<TransientHolon> = Vec::new();

        // Direct traversal from LoaderHolon → LoaderRelationshipReference entries.
        let rel_name = N::rel(N::REL_HAS_LOADER_REL_REF);
        let coll = loader.get_related_holons(context, &rel_name)?;

        coll.is_accessible(AccessType::Read)?;
        let members = coll.get_members();

        for h_ref in members {
            // Work on detached copies so Pass-2 can resolve in any order.
            let loader_rel = h_ref.clone_holon(context)?;
            out.push(loader_rel);
        }

        Ok(out)
    }
}

