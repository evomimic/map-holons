// crates/holons_core/src/holon_loader/loader_ref_resolver.rs
//
// Pass-2: Resolve queued LoaderRelationshipReference holons into concrete writes
// on staged holons (declared links and/or DescribedBy).
//
// Design principles:
// - Non-fatal errors are collected, not thrown
// - Deduplicate within run (avoid writing identical edges twice)
// - Centralize metaschema coupling (inverse↔declared lookup lives here)
// - Stage or promote only write-sources that already exist; never inline new holons
//
// Invariants & Guarantees:
// - All writes occur on **staged** holons.
// - If a write-source holon is **saved**, we stage a new version on demand
//   before writing (see `resolve_staged_write_source()`).
// - Inline / embedded holon definitions are **not** created here;
//   such malformed inputs must have been filtered or rejected earlier.
// - The resolver is idempotent within one run: duplicate (source, rel, target)
//   triples are skipped via `RelationshipEdgeKey` deduplication.

use std::collections::{HashMap, HashSet};

use crate::reference_layer::{
    HolonReference, HolonsContextBehavior, ReadableHolon, SmartReference, StagedReference,
    TransientReference, WritableHolon,
};
use crate::stage_new_version_api;
use crate::HolonCollectionApi;
use base_types::{BaseValue, MapBoolean, MapString};
use core_types::{HolonError, PropertyName, RelationshipName};
use tracing::{debug, info, instrument, warn};
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, ToRelationshipName};

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Outcome of Pass-2: counts successful writes and collects non-fatal errors.
/// The controller decides whether to commit based on `errors.is_empty()`.
#[derive(Debug, Default)]
pub struct ResolverOutcome {
    /// Total number of links scheduled on staged holons
    pub links_created: i64,
    /// Non-fatal errors encountered during resolution
    pub errors: Vec<HolonError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RelationshipEdgeKey {
    /// Best-effort stable identifier for the write source
    /// (holon_id > versioned_key > key > "<no-id>")
    source_identifier: String,
    /// Declared (forward) relationship name under which the link will be written
    relationship_name: RelationshipName,
    /// Best-effort stable identifier for the write target (same strategy as source)
    target_identifier: String,
}

/// Pass-2 resolver: turns queued LoaderRelationshipReference holons
/// into concrete writes on staged holons (declared links and/or DescribedBy).
///
/// Each call:
/// 1. Collects queued references from Pass-1
/// 2. Ensures all write-sources are staged and writable
/// 3. Deduplicates edges within the run
/// 4. Writes relationships (including DescribedBy) onto staged holons
/// 5. Aggregates non-fatal errors for the controller to decide commit policy
pub struct LoaderRefResolver;

impl LoaderRefResolver {
    /// Resolve all queued LoaderRelationshipReference holons created during Pass-1
    /// and write them onto staged holons. Returns `Ok(ResolverOutcome)` even if
    /// per-item errors occurred (they’re aggregated in `errors`). Only returns
    /// `Err(...)` for systemic failures that prevent processing the inputs.
    #[instrument(level = "info", skip_all)]
    pub fn resolve_relationships(
        context: &dyn HolonsContextBehavior,
        queued_relationship_references: Vec<TransientReference>,
    ) -> Result<ResolverOutcome, HolonError> {
        let mut outcome = ResolverOutcome::default();

        // Per-call cache for inverse→declared results.
        // 1) Seed from core/meta (feature-gated)
        // 2) Extend with pairs asserted in this load (handles “no links written yet”)
        let mut inverse_to_declared_name_cache: HashMap<RelationshipName, RelationshipName> =
            HashMap::new(); // Optionally seed metaschema relationship pairs

        let derived_from_queue =
            Self::index_inverse_pairs_from_queued(context, &queued_relationship_references);
        inverse_to_declared_name_cache.extend(derived_from_queue);

        // Deduplicate within this run using a self-describing key
        let mut seen_relationship_edge_keys: HashSet<RelationshipEdgeKey> = HashSet::new();

        for relationship_reference in queued_relationship_references {
            match Self::resolve_single_reference(
                context,
                &relationship_reference,
                &mut inverse_to_declared_name_cache,
                &mut seen_relationship_edge_keys,
            ) {
                Ok(created) => outcome.links_created += created,
                Err(error) => outcome.errors.push(error),
            }
        }

        info!(
            "Pass-2 complete: links_created={}, errors={}",
            outcome.links_created,
            outcome.errors.len()
        );

        Ok(outcome)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private: Main resolution flow
    // ─────────────────────────────────────────────────────────────────────────

    /// Resolve a single LoaderRelationshipReference into concrete writes.
    /// Returns the number of edges actually written (descriptor counts as 1).
    #[instrument(level = "debug", skip_all)]
    fn resolve_single_reference(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
        inverse_to_declared_name_cache: &mut HashMap<RelationshipName, RelationshipName>,
        seen_relationship_edge_keys: &mut HashSet<RelationshipEdgeKey>,
    ) -> Result<i64, HolonError> {
        // 1) Metadata
        let (supplied_relationship_name, is_declared_relationship) =
            Self::extract_relationship_metadata(context, relationship_reference)?;

        // 2) Endpoints: one source, many targets
        let (reference_source_endpoint, reference_target_endpoints) =
            Self::resolve_endpoints(context, relationship_reference)?;

        let declared_relationship_name = if is_declared_relationship {
            supplied_relationship_name
        } else {
            Self::get_declared_for_inverse_cached(
                context,
                &supplied_relationship_name,
                inverse_to_declared_name_cache,
            )?
        };

        // 3) Declared vs inverse path
        if is_declared_relationship {
            // Resolve staged source once (declared orientation uses the LRR source)
            let staged_write_source =
                Self::resolve_staged_write_source(context, &reference_source_endpoint)?;

            // Compute the write-source identifier once for this batch
            let source_ref_for_dedupe = HolonReference::Staged(staged_write_source.clone());
            let source_identifier_once =
                Self::best_identifier_for_dedupe(context, &source_ref_for_dedupe);

            // Per-edge dedupe: keep only unique (source, rel, target) edges
            let mut filtered_targets = Vec::with_capacity(reference_target_endpoints.len());
            for target_reference in reference_target_endpoints {
                let edge_key = RelationshipEdgeKey {
                    source_identifier: source_identifier_once.clone(),
                    relationship_name: declared_relationship_name.clone(),
                    target_identifier: Self::best_identifier_for_dedupe(context, &target_reference),
                };

                if seen_relationship_edge_keys.insert(edge_key) {
                    filtered_targets.push(target_reference);
                } else {
                    debug!(
                        "Duplicate relationship skipped (declared path): source={}, rel={}, target=<deduped>",
                        source_identifier_once,
                        declared_relationship_name,
                    );
                }
            }

            // Delegate the actual write (handles descriptor validation + count)
            Self::write_relationship(
                context,
                staged_write_source,
                &declared_relationship_name,
                filtered_targets,
            )
        } else {
            // Inverse path: map to declared and flip direction (each target → source)
            let mut links_created: i64 = 0;

            // Precompute identifier for the (declared) target side (the original LRR source)
            let declared_target_id_once =
                Self::best_identifier_for_dedupe(context, &reference_source_endpoint);

            for target_endpoint in reference_target_endpoints {
                // Each target becomes the write source in declared orientation
                let staged_write_source =
                    Self::resolve_staged_write_source(context, &target_endpoint)?;

                // Compute this (declared) source identifier once for this edge
                let declared_source_id_once = Self::best_identifier_for_dedupe(
                    context,
                    &HolonReference::Staged(staged_write_source.clone()),
                );

                let edge_key = RelationshipEdgeKey {
                    source_identifier: declared_source_id_once.clone(),
                    relationship_name: declared_relationship_name.clone(),
                    target_identifier: declared_target_id_once.clone(),
                };

                if seen_relationship_edge_keys.insert(edge_key) {
                    // Single-target vector: original source becomes the only target
                    links_created += Self::write_relationship(
                        context,
                        staged_write_source,
                        &declared_relationship_name,
                        vec![reference_source_endpoint.clone()],
                    )?;
                    debug!(
                        "Created relationship (inverse path → declared): source={}, rel={}, target={}",
                        declared_source_id_once,
                        declared_relationship_name,
                        declared_target_id_once,
                    );
                } else {
                    debug!(
                        "Duplicate relationship skipped (inverse path): source={}, rel={}, target={}",
                        declared_source_id_once,
                        declared_relationship_name,
                        declared_target_id_once,
                    );
                }
            }

            Ok(links_created)
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private: Step-by-step helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Extract relationship_name and is_declared from LoaderRelationshipReference.
    fn extract_relationship_metadata(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
    ) -> Result<(RelationshipName, bool), HolonError> {
        let relationship_name_property: PropertyName =
            CorePropertyTypeName::RelationshipName.as_property_name();
        let is_declared_property: PropertyName =
            CorePropertyTypeName::IsDeclared.as_property_name();

        let relationship_value = relationship_reference
            .property_value(context, &relationship_name_property)?
            .ok_or_else(|| {
                HolonError::EmptyField("LoaderRelationshipReference.RelationshipName".into())
            })?;

        let relationship_name = match relationship_value {
            BaseValue::StringValue(MapString(text)) => text.to_relationship_name(),
            other => {
                return Err(HolonError::UnexpectedValueType(format!("{other:?}"), "String".into()))
            }
        };

        let is_declared_value =
            relationship_reference.property_value(context, &is_declared_property)?.ok_or_else(
                || HolonError::EmptyField("LoaderRelationshipReference.IsDeclared".into()),
            )?;

        let is_declared_flag: bool = match is_declared_value {
            BaseValue::BooleanValue(MapBoolean(inner)) => inner, // convert to plain bool
            other => {
                return Err(HolonError::UnexpectedValueType(format!("{other:?}"), "bool".into()))
            }
        };

        Ok((relationship_name, is_declared_flag))
    }

    /// Ensure that `ReferenceSource` is a singleton and collect `ReferenceTarget` endpoints
    /// for a given LoaderRelationshipReference; return their HolonReferences.
    fn resolve_endpoints(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
    ) -> Result<(HolonReference, Vec<HolonReference>), HolonError> {
        let src_rel = CoreRelationshipTypeName::ReferenceSource.as_relationship_name();
        let tgt_rel = CoreRelationshipTypeName::ReferenceTarget.as_relationship_name();

        let sources = relationship_reference.related_holons(context, src_rel)?;
        let targets = relationship_reference.related_holons(context, tgt_rel)?;

        match sources.get_count().0 {
            1 => {}
            0 => {
                return Err(HolonError::EmptyField(
                    "LoaderRelationshipReference.ReferenceSource".into(),
                ))
            }
            n => {
                return Err(HolonError::DuplicateError(
                    "ReferenceSource".into(),
                    format!("{n} found"),
                ))
            }
        }

        let target_count = targets.get_count().0;
        if target_count < 1 {
            return Err(HolonError::EmptyField(
                "LoaderRelationshipReference.ReferenceTarget".into(),
            ));
        }

        let source = sources.get_by_index(0)?;
        let mut target_vec = Vec::with_capacity(target_count as usize);
        for i in 0..target_count {
            target_vec.push(targets.get_by_index(i as usize)?);
        }
        Ok((source, target_vec))
    }

    /// Ensure the write-source is a writable staged holon.
    ///
    /// Policy:
    /// - If already staged → return it.
    /// - Else if saved (has HolonId) → stage a **new version** (`stage_new_version_api`) and return it.
    /// - Else (e.g., inline/transient with no persistent identity) → error; Pass-2 won't invent embedded holons.
    fn resolve_staged_write_source(
        context: &dyn HolonsContextBehavior,
        write_source_endpoint: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
        let staging_access = context.get_space_manager().get_staging_behavior_access();

        // 1) If the endpoint already corresponds to a staged holon, use it (prefer unique versioned key).
        if let Ok(vkey) = write_source_endpoint.versioned_key(context) {
            if let Ok(staged) = staging_access.borrow().get_staged_holon_by_versioned_key(&vkey) {
                return Ok(staged);
            }
        }
        if let Ok(Some(base_key)) = write_source_endpoint.key(context) {
            let staged_matches =
                staging_access.borrow().get_staged_holons_by_base_key(&base_key)?;
            match staged_matches.len() {
                1 => return Ok(staged_matches.into_iter().next().unwrap()),
                n if n > 1 => {
                    return Err(HolonError::DuplicateError(
                        "write source by base key".into(),
                        n.to_string(),
                    ))
                }
                _ => {} // not staged by base key; try promotion next
            }
        }

        // 2) Promotion path: saved → staged new version (requires HolonId).
        if let Ok(saved_id) = write_source_endpoint.holon_id(context) {
            // Build a SmartReference by id and stage a new version via the public API.
            let smart = SmartReference::new_from_id(saved_id);
            let staged = stage_new_version_api(context, smart)?;
            return Ok(staged);
        }

        // 3) No staged match and no saved identity → not supported in Pass-2.
        Err(HolonError::InvalidParameter(
            "Write source is not staged, and no saved identity (holon_id) available to stage a new version. Inline/embedded instance creation is not supported in Pass-2."
                .into(),
        ))
    }

    /// Construct a RelationshipEdgeKey from references and a relationship name.
    /// Uses best-available identifiers to ensure deterministic deduplication.
    fn make_edge_key(
        context: &dyn HolonsContextBehavior,
        source_ref: &HolonReference,
        relationship_name: &RelationshipName,
        target_ref: &HolonReference,
    ) -> RelationshipEdgeKey {
        RelationshipEdgeKey {
            source_identifier: Self::best_identifier_for_dedupe(context, source_ref),
            relationship_name: relationship_name.clone(),
            target_identifier: Self::best_identifier_for_dedupe(context, target_ref),
        }
    }

    /// Build inverse→declared mapping from *this batch* by scanning LoaderRelationshipReferences
    /// that assert `InverseOf` (canonical) and, optionally, `HasInverse` (fallback).
    /// Read-light strategy:
    ///  - Phase A: prefilter by (IsDeclared, RelationshipName) with no endpoint derefs
    ///  - Phase B: resolve only candidates, with per-run memoization of endpoint→TypeName
    fn index_inverse_pairs_from_queued(
        context: &dyn HolonsContextBehavior,
        queued: &[TransientReference],
    ) -> HashMap<RelationshipName, RelationshipName> {
        let mut map = HashMap::new();

        // Memoize endpoint -> TypeName using either HolonKey or HolonId as cache key.
        let mut endpoint_typename_cache: HashMap<String, RelationshipName> = HashMap::new();

        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();
        let has_inverse = CoreRelationshipTypeName::HasInverse.as_relationship_name();

        // Phase A: prefilter candidates without touching endpoints
        let mut inverse_of_candidates: Vec<&TransientReference> = Vec::new();
        let mut has_inverse_candidates: Vec<&TransientReference> = Vec::new();

        for lrr in queued {
            let Ok((rel_name, is_declared)) = Self::extract_relationship_metadata(context, lrr)
            else {
                continue;
            };
            if is_declared && rel_name == inverse_of {
                inverse_of_candidates.push(lrr);
            } else if !is_declared && rel_name == has_inverse {
                // Optional fallback: inverse-oriented LRR; polarity will be flipped later
                has_inverse_candidates.push(lrr);
            }
        }

        // Helper: derive a stable cache key for an endpoint (prefer HolonKey, else HolonId)
        let cache_key_for = |ep: &HolonReference| -> Option<String> {
            let key_prop: PropertyName = CorePropertyTypeName::HolonKey.as_property_name();
            let id_prop: PropertyName = CorePropertyTypeName::HolonId.as_property_name();
            if let Ok(Some(BaseValue::StringValue(MapString(k)))) =
                ep.property_value(context, &key_prop)
            {
                return Some(format!("key:{k}"));
            }
            if let Ok(Some(BaseValue::StringValue(MapString(i)))) =
                ep.property_value(context, &id_prop)
            {
                return Some(format!("id:{i}"));
            }
            None
        };

        // Helper: memoized TypeName read for a Loader endpoint
        let mut typename_for_endpoint = |ep: &HolonReference| -> Option<RelationshipName> {
            if let Some(ck) = cache_key_for(ep) {
                if let Some(name) = endpoint_typename_cache.get(&ck) {
                    return Some(name.clone());
                }
                match Self::get_type_name_from_loader_endpoint(context, ep) {
                    Ok(Some(name)) => {
                        endpoint_typename_cache.insert(ck, name.clone());
                        Some(name)
                    }
                    _ => None,
                }
            } else {
                match Self::get_type_name_from_loader_endpoint(context, ep) {
                    Ok(Some(name)) => Some(name),
                    _ => None,
                }
            }
        };

        // Phase B.1: process canonical INVERSE_OF (declared) first
        for lrr in inverse_of_candidates {
            let Ok((src, tgts)) = Self::resolve_endpoints(context, lrr) else { continue };
            if tgts.len() != 1 {
                continue;
            }
            let tgt = &tgts[0];

            let Some(inverse_name) = typename_for_endpoint(&src) else { continue };
            // If this inverse already mapped, skip work entirely
            if map.contains_key(&inverse_name) {
                continue;
            }

            let Some(declared_name) = typename_for_endpoint(tgt) else { continue };

            if let Some(existing) = map.get(&inverse_name) {
                if existing != &declared_name {
                    warn!(
                    "Conflicting inverse mapping in batch (InverseOf): inverse='{}' declared='{}' vs '{}'",
                    inverse_name, existing, declared_name
                );
                    continue; // keep first
                }
            } else {
                map.insert(inverse_name, declared_name);
            }
        }

        // Phase B.2: optional fallback — process HAS_INVERSE (inverse-oriented, IsDeclared=false)
        for lrr in has_inverse_candidates {
            let Ok((src, tgts)) = Self::resolve_endpoints(context, lrr) else { continue };
            if tgts.len() != 1 {
                continue;
            }
            let tgt = &tgts[0];

            // Polarity flip: (DeclaredType) -[HasInverse]-> (InverseType)
            let Some(declared_name) = typename_for_endpoint(&src) else { continue };
            let Some(inverse_name) = typename_for_endpoint(tgt) else { continue };

            if map.contains_key(&inverse_name) {
                // Prefer InverseOf-derived mapping; skip fallback
                continue;
            }
            map.insert(inverse_name, declared_name);
        }

        if !map.is_empty() {
            info!(
            "Indexed {} inverse↔declared pairs from batch ({} INVERSE_OF, {} HAS_INVERSE, {} cached endpoints)",
            map.len(),
            inverse_of_candidates.len(),
            has_inverse_candidates.len(),
            endpoint_typename_cache.len()
        );
        }

        map
    }

    /// Resolve a Loader endpoint to the *real* holon and read its TypeName.
    ///
    /// Resolution order:
    /// 1) `HolonKey` → staged lookup in Nursery (fast path during loads)
    /// 2) `HolonId`  → read via SmartReference (saved/cached)
    ///
    /// Returns the descriptor's TypeName as a RelationshipName if found.
    fn get_type_name_from_loader_endpoint(
        context: &dyn HolonsContextBehavior,
        endpoint: &HolonReference,
    ) -> Result<Option<RelationshipName>, HolonError> {
        let key_prop: PropertyName = CorePropertyTypeName::HolonKey.as_property_name();
        let id_prop: PropertyName = CorePropertyTypeName::HolonId.as_property_name();
        let type_prop: PropertyName = CorePropertyTypeName::TypeName.as_property_name();

        // 1) Try staged-by-key
        if let Some(BaseValue::StringValue(MapString(base_key))) =
            endpoint.property_value(context, &key_prop)?
        {
            let staging = context.get_space_manager().get_staging_behavior_access();
            let staged =
                staging.borrow().get_staged_holons_by_base_key(&MapString(base_key.clone()))?;
            if staged.len() == 1 {
                let staged_ref = HolonReference::Staged(staged.into_iter().next().unwrap());
                if let Some(BaseValue::StringValue(MapString(name))) =
                    staged_ref.property_value(context, &type_prop)?
                {
                    return Ok(Some(name.to_relationship_name()));
                }
            }
        }

        // 2) Fallback: read directly from saved by HolonId
        if let Some(BaseValue::StringValue(MapString(holon_id))) =
            endpoint.property_value(context, &id_prop)?
        {
            let smart = SmartReference::new_from_id(holon_id);
            if let Some(BaseValue::StringValue(MapString(name))) =
                smart.property_value(context, &type_prop)?
            {
                return Ok(Some(name.to_relationship_name()));
            }
        }

        Ok(None)
    }

    /// Perform the write on the staged source and return the number of edges created.
    /// - For DescribedBy: requires exactly one target; returns 1 on success, 0 if no target.
    /// - For other relationships: batches the provided targets; returns targets.len().
    fn write_relationship(
        context: &dyn HolonsContextBehavior,
        staged_source: StagedReference,
        declared_relationship_name: &RelationshipName,
        mut write_targets: Vec<HolonReference>,
    ) -> Result<i64, HolonError> {
        let is_descriptor = *declared_relationship_name
            == CoreRelationshipTypeName::DescribedBy.as_relationship_name();

        if is_descriptor {
            match write_targets.len() {
                0 => {
                    // Nothing to do (e.g., everything deduped away)
                    return Ok(0);
                }
                1 => {
                    staged_source.with_descriptor(context, write_targets.remove(0))?;
                    return Ok(1);
                }
                _ => {
                    return Err(HolonError::InvalidRelationship(
                        declared_relationship_name.to_string(),
                        "DescribedBy target was duplicate or ambiguous; expected exactly one unique target"
                            .into(),
                    ));
                }
            }
        }

        if write_targets.is_empty() {
            return Ok(0);
        }

        let number_of_targets = write_targets.len() as i64;
        staged_source.add_related_holons(
            context,
            declared_relationship_name.clone(),
            write_targets,
        )?;
        Ok(number_of_targets)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private: Utility helpers
    // ─────────────────────────────────────────────────────────────────────────

    // ─────────────────────────────────────────────────────────────────────
    // On-demand inverse resolution (with per-run cache)
    // ─────────────────────────────────────────────────────────────────────

    /// Get the declared counterpart for an inverse name:
    /// 1) check the cache; 2) resolve from type graph (single lookup); 3) cache and return.
    fn get_declared_for_inverse_cached(
        context: &dyn HolonsContextBehavior,
        inverse_name: &RelationshipName,
        cache: &mut HashMap<RelationshipName, RelationshipName>,
    ) -> Result<RelationshipName, HolonError> {
        if let Some(declared) = cache.get(inverse_name) {
            return Ok(declared.clone());
        }

        // Resolve just this inverse name and cache the result on success.
        match Self::resolve_declared_from_inverse_on_demand(context, inverse_name)? {
            Some(declared) => {
                info!("Resolved inverse '{}' → declared '{}'", inverse_name, declared);
                cache.insert(inverse_name.clone(), declared.clone());
                Ok(declared)
            }
            None => {
                warn!("Failed to resolve declared counterpart for inverse '{}'", inverse_name);
                Err(HolonError::InvalidRelationship(
                    inverse_name.to_string(),
                    "no declared counterpart found".into(),
                ))
            }
        }
    }

    /// Resolve a single inverse name by walking the *live* metaschema (saved/staged):
    /// iterate DeclaredRelationshipType descriptors, follow `HasInverse` to the inverse,
    /// compare the inverse's `TypeName` to `inverse_name`, and return the declared `TypeName` on match.
    fn resolve_declared_from_inverse_on_demand(
        context: &dyn HolonsContextBehavior,
        inverse_name: &RelationshipName,
    ) -> Result<Option<RelationshipName>, HolonError> {
        let type_name_prop: PropertyName = CorePropertyTypeName::TypeName.as_property_name();
        let has_inverse_rel = CoreRelationshipTypeName::HasInverse.as_relationship_name();

        // Unimplemented
        Ok(None)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Diagnostics and identity helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Best-effort identifier for dedupe/diagnostics:
    /// prefer `holon_id`; else `versioned_key`; else `key`; else `"<no-id>"`.
    fn best_identifier_for_dedupe(
        context: &dyn HolonsContextBehavior,
        reference: &HolonReference,
    ) -> String {
        // Strongest, stable across versions
        if let Ok(id) = reference.holon_id(context) {
            return format!("id:{id}");
        }

        // Next best: includes version; good for staged/transient instances
        if let Ok(vk) = reference.versioned_key(context) {
            return format!("vkey:{vk}");
        }

        // Base key (may be absent for some transients)
        if let Ok(Some(k)) = reference.key(context) {
            return format!("key:{k}");
        }

        // Stable fallback so dedupe set still behaves deterministically
        "<no-id>".to_string()
    }
}
