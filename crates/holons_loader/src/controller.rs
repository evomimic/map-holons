// crates/holons_loader/src/controller.rs
//
// Orchestrates the two-pass holon loading flow:
//
//   Pass 1: Map & stage node holons (properties only); queue relationship references.
//   Pass 2: Resolve queued edges to concrete declared links (declared, inverse, DescribedBy).
//   Commit: Persist staged holons in one bulk commit.
//   Respond: Return a *transient* HolonLoadResponse (with related *transient* HolonLoadError holons).
//
// This controller keeps only per-call, in-memory state (no cross-call persistence).
// It is intentionally thin: it wires together Mapper → Resolver → Commit → Response.

use std::collections::HashMap;
use tracing::{debug, info, warn};
// use uuid::Uuid;

use holons_prelude::prelude::*;

use crate::errors::{make_error_holons_best_effort, ErrorWithContext};
use crate::{LoaderHolonMapper, LoaderRefResolver, ResolverOutcome};

/// Local structure to hold LoaderHolon provenance used for error enrichment.
#[derive(Debug, Clone)]
pub struct FileProvenance {
    pub filename: MapString,
    pub start_utf8_byte_offset: Option<i64>,
}

pub type ProvenanceIndex = HashMap<MapString /* loader_holon_key */, FileProvenance>;

/// HolonLoaderController: top-level coordinator for the loader pipeline.
#[derive(Debug, Default)]
pub struct HolonLoaderController;

impl HolonLoaderController {
    /// Create a new controller with empty per-call caches.
    pub fn new() -> Self {
        Self
    }

    /// Entry point called by the Guest-side adapter.
    ///
    /// Inputs:
    /// - `context`: guest-side reference-layer context (Nursery, Cache, managers)
    /// - `set`: a *transient* HolonLoadSet that CONTAINS → HolonLoaderBundles
    ///   Note: Each bundle contains LoaderHolons parsed from a single input file and MUST
    ///   include a "Filename" property for diagnostics.
    ///
    /// Output:
    /// - `Ok(TransientReference)` to a *transient* HolonLoadResponse (message-only)
    /// - `Err(HolonError)` for system-level failures preventing any meaningful response
    ///   Note: The response holon contains any per-item errors as related HolonLoadError holons,
    ///   enriched with filename and start byte offset when available.
    ///
    pub fn load_set(
        &mut self,
        context: &dyn HolonsContextBehavior,
        set_reference: TransientReference, // -> HolonLoadSet
    ) -> Result<TransientReference, HolonError> {
        // let run_id = Uuid::new_v4();
        // info!("HolonLoaderController::load_set - start run_id={run_id}");
        let run_id = 1; // Temporary fixed run_id until we wire in Uuid

        // ─────────────────────────────────────────────────────────────────────
        // Discover all HolonLoaderBundle references in the HolonLoadSet
        // ─────────────────────────────────────────────────────────────────────
        let bundle_references: Vec<TransientReference> =
            Self::discover_bundle_transients(context, &set_reference)?;

        let total_bundles = bundle_references.len() as i64;

        if total_bundles == 0 {
            info!("HolonLoaderController::load_set - early return (empty set: no bundles)");

            let summary = "Empty set: no HolonLoaderBundles found; nothing to process.".to_string();

            let response_reference = self.build_response(
                context,
                run_id,
                0, // holons_staged
                0, // holons_committed
                0, // links_created
                0, // errors_encountered
                total_bundles,
                0, // total_loader_holons
                summary,
                Vec::new(), // no error holons
            )?;

            info!("HolonLoaderController::load_set - done (empty set)");
            return Ok(response_reference);
        }

        // ─────────────────────────────────────────────────────────────────────
        // PASS 1: map & stage node holons (properties only); queue relationship refs
        //         across ALL bundles in a unified staging context
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_set - pass1_stage_all_bundles");

        let mut total_loader_holons = 0;
        let mut total_holons_staged = 0;
        let mut merged_queued_relationship_references: Vec<TransientReference> = Vec::new();

        // In-memory provenance index: loader_holon_key -> (filename, start_utf8_byte_offset)
        let mut provenance_index: ProvenanceIndex = HashMap::new();
        // Accumulates duplicate-key errors discovered while indexing provenance.
        let mut provenance_errors: Vec<ErrorWithContext> = Vec::new();

        // Collect provenance data from all bundles first
        for bundle_reference in bundle_references.iter() {
            // Read required "Filename" property from the bundle
            let filename = Self::read_required_string_property(
                context,
                bundle_reference,
                CorePropertyTypeName::Filename,
            )?;

            // Index provenance from this bundle's LoaderHolons
            self.collect_provenance_from_bundle(
                context,
                bundle_reference,
                &filename,
                &mut provenance_index,
                &mut provenance_errors,
            )?;

            // Map & stage LoaderHolons from this bundle; queue its relationship references
            let mut mapper_output =
                LoaderHolonMapper::map_bundle(context, bundle_reference.clone())?;

            total_holons_staged += mapper_output.staged_count;
            total_loader_holons += mapper_output.loader_holon_count;

            merged_queued_relationship_references
                .extend(std::mem::take(&mut mapper_output.queued_relationship_references));

            // SHORT-CIRCUIT CASE 1:
            // If Pass 1 produces any errors, short-circuit now (skip Pass 2 and commit).
            if !mapper_output.errors.is_empty() {
                let pass1_error_count = mapper_output.errors.len() as i64;

                warn!(
                    "HolonLoaderController::load_set - early return due to Pass 1 errors ({} detected)",
                    pass1_error_count
                );

                // Prefer typed error holons; enrich with filename/offset via provenance index.
                let error_holons = make_error_holons_best_effort(
                    context,
                    &mapper_output.errors,
                    Some(&provenance_index),
                )?;

                let summary = format!(
                    "Pass 1 reported {} error(s). Pass 2 and commit were skipped.",
                    pass1_error_count
                );

                let response_reference = self.build_response(
                    context,
                    run_id,
                    total_holons_staged,
                    0,                 // holons_committed
                    0,                 // links_created
                    pass1_error_count, // always use real error count, not holon count
                    total_bundles,
                    total_loader_holons,
                    summary,
                    error_holons,
                )?;

                warn!("HolonLoaderController::load_set - done (aborted after Pass 1)");
                return Ok(response_reference);
            }
        }

        // SHORT-CIRCUIT CASE 2:
        // Duplicate loader_holon keys across the set: treat as a hard Pass-1 failure.
        if !provenance_errors.is_empty() {
            let duplicate_error_count = provenance_errors.len() as i64;

            warn!(
                "HolonLoaderController::load_set - duplicate loader_holon keys detected ({} error(s)); \
                 Pass 2 and commit will be skipped",
                duplicate_error_count
            );

            // Build error holons, enriched with filename/offset via provenance_index.
            let error_holons = make_error_holons_best_effort(
                context,
                &provenance_errors,
                Some(&provenance_index),
            )?;

            let summary = format!(
                "Duplicate loader_holon keys detected across HolonLoadSet. \
                 Pass 2 and commit were skipped. {} holons staged; 0 committed.",
                total_holons_staged
            );

            let response_reference = self.build_response(
                context,
                run_id,
                total_holons_staged,
                0,                     // holons_committed
                0,                     // links_created (none attempted)
                duplicate_error_count, // errors_encountered
                total_bundles,
                total_loader_holons,
                summary,
                error_holons,
            )?;

            warn!("HolonLoaderController::load_set - done (aborted due to duplicate keys)");
            return Ok(response_reference);
        }

        // SHORT-CIRCUIT CASE 3:
        // Empty set short-circuit check: nothing staged and no relationships queued
        let no_staged_holons = total_holons_staged == 0;
        let no_relationships = merged_queued_relationship_references.is_empty();
        let is_wholly_empty_set = no_staged_holons && no_relationships;

        if is_wholly_empty_set {
            info!(
                "HolonLoaderController::load_set - early return (empty set: no holons, no relationships)"
            );

            let summary =
                "Empty set: no LoaderHolons or relationship references found; nothing to process."
                    .to_string();

            let response_reference = self.build_response(
                context,
                run_id,
                0, // holons_staged
                0, // holons_committed
                0, // links_created
                0, // errors_encountered
                total_bundles,
                total_loader_holons,
                summary,
                Vec::new(), // no error holons
            )?;

            info!("HolonLoaderController::load_set - done (empty set short-circuit)");
            return Ok(response_reference);
        }

        // ─────────────────────────────────────────────────────────────────────
        // PASS 2: resolve queued references and write declared links (across the set)
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_set - pass2_resolve_all");

        let ResolverOutcome { links_created, errors: resolver_errors } =
            LoaderRefResolver::resolve_relationships(
                context,
                merged_queued_relationship_references,
            )?;

        // SHORT-CIRCUIT CASE 3:
        // If Pass 2 produced any errors, build the response now and return (skip commit).
        if !resolver_errors.is_empty() {
            let resolver_error_count = resolver_errors.len() as i64;

            warn!(
                "HolonLoaderController::load_set - pass2 errors ({}), short-circuit before commit",
                resolver_error_count
            );

            let error_holons =
                make_error_holons_best_effort(context, &resolver_errors, Some(&provenance_index))?;

            let response_reference = self.build_response(
                context,
                run_id,
                total_holons_staged,
                0, // holons_committed
                links_created,
                resolver_error_count,
                total_bundles,
                total_loader_holons,
                format!(
                    "Pass 2 reported {} error(s). Commit was skipped. {} holons staged; 0 committed; {} links attempted.",
                    resolver_error_count, total_holons_staged, links_created
                ),
                error_holons,
            )?;

            warn!("HolonLoaderController::load_set - done (pass2 short-circuit)");
            return Ok(response_reference);
        }

        // ─────────────────────────────────────────────────────────────────────
        // COMMIT: persist all staged holons (only if both phases succeeded)
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_set - commit");

        // commit(): provided by HolonOperationsApi via holons_prelude
        let commit_response = commit(context)?;
        // Basic accounting:
        // - All staged nursery holons are attempted.
        // - Abandoned are not saved; they appear in `abandoned_holons`.
        // - If saved + abandoned != commits_attempted, then errors occurred.
        let holons_committed = commit_response.saved_holons.len() as i64;
        let holons_abandoned = commit_response.abandoned_holons.len() as i64;
        let commits_attempted = commit_response.commits_attempted.into();

        let commit_ok = (holons_committed + holons_abandoned) == commits_attempted;

        let summary = if commit_ok {
            format!(
                "Commit successful: {} holons staged; {} committed; {} abandoned; {} attempts.",
                total_holons_staged, holons_committed, holons_abandoned, commits_attempted
            )
        } else {
            format!(
                "Commit incomplete: {} holons staged; {} committed; {} abandoned; {} attempts.",
                total_holons_staged, holons_committed, holons_abandoned, commits_attempted
            )
        };

        // We’re not surfacing per-item commit errors yet; just report via summary.
        let response_reference = self.build_response(
            context,
            run_id,
            total_holons_staged,
            holons_committed,
            links_created,
            0, // errors_encountered
            total_bundles,
            total_loader_holons,
            summary,
            Vec::new(),
        )?;

        debug!("HolonLoaderController::load_set - done");
        Ok(response_reference)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Discover HolonLoaderBundle references from a HolonLoadSet.
    /// - Uses `related_holons()` and manages the RwLock explicitly (current TransientHolonManager behavior).
    /// - Holds the read lock while iterating members to avoid cloning the HolonCollection.
    ///
    /// Safety note:
    ///   The bundle/set relationship graph is immutable once parsing completes (no active writers in the loader phase).
    ///   Retaining the read lock during iteration is safe *because* no writers exist on the load set in this phase.
    ///
    /// Relationship: (HolonLoadSet)-[CONTAINS]->(HolonLoaderBundle)
    fn discover_bundle_transients(
        context: &dyn HolonsContextBehavior,
        set_reference: &TransientReference,
    ) -> Result<Vec<TransientReference>, HolonError> {
        info!("discover_bundle_transients: fetching CONTAINS collection from HolonLoadSet");

        let relationship_type = CoreRelationshipTypeName::Contains;
        let collection_handle = match set_reference.related_holons(context, &relationship_type) {
            Ok(handle) => handle,
            Err(e) => {
                // If the relationship is absent, treat as empty rather than a hard error.
                warn!(
                "discover_bundle_transients: no CONTAINS relationship found on set (treating as empty): {}",
                e
            );
                return Ok(Vec::new());
            }
        };

        // Hold the read lock while accessing members. No writers exist during the loader phase.
        let out: Vec<TransientReference> = {
            let guard = match collection_handle.read() {
                Ok(g) => g,
                Err(_) => {
                    warn!("discover_bundle_transients: failed to read HolonCollection (poisoned lock)");
                    return Ok(Vec::new());
                }
            };

            let members = guard.get_members();
            if members.is_empty() {
                info!("discover_bundle_transients: set has zero bundles");
                return Ok(Vec::new());
            }

            let mut transients = Vec::with_capacity(members.len());
            for holon_reference in members.iter() {
                match holon_reference {
                    // Assumption per loader design: all bundle members are transient.
                    HolonReference::Transient(transient_reference) => {
                        transients.push(transient_reference.clone())
                    }
                    _ => {
                        // Defensive: log and skip if the assumption is ever violated.
                        warn!("discover_bundle_transients: unexpected non-transient member encountered; skipping");
                    }
                }
            }
            transients
        };

        Ok(out)
    }

    /// Collect per-loader provenance for a single bundle by scanning its member LoaderHolons.
    /// Best-effort: missing key or offset is tolerated; offset defaults to 0.
    ///
    /// Locking & safety:
    ///   The bundle’s relationship map is immutable once parsing completes (loader phase has no writers).
    ///   It is therefore safe to hold the read lock while iterating members to avoid cloning.
    ///
    /// Relationship: (HolonLoaderBundle)-[BUNDLE_MEMBERS]->(LoaderHolon)
    fn collect_provenance_from_bundle(
        &self,
        context: &dyn HolonsContextBehavior,
        bundle_reference: &TransientReference,
        filename: &MapString,
        provenance_index: &mut ProvenanceIndex,
        duplicate_errors: &mut Vec<ErrorWithContext>,
    ) -> Result<(), HolonError> {
        let members_relationship = CoreRelationshipTypeName::BundleMembers;

        let collection_handle =
            match bundle_reference.related_holons(context, &members_relationship) {
                Ok(handle) => handle,
                Err(_) => {
                    // No members → nothing to index.
                    return Ok(());
                }
            };

        // Hold the read lock while iterating (no writers exist during the loader phase).
        let guard = collection_handle.read().map_err(|_| {
            HolonError::FailedToBorrow("HolonCollection read (bundle members)".into())
        })?;

        let members = guard.get_members();
        if members.is_empty() {
            return Ok(());
        }

        for holon_reference in members.iter() {
            // Assumption per loader design: bundle members are transient.
            let transient_member_reference = match holon_reference {
                HolonReference::Transient(transient_reference) => transient_reference,
                _ => {
                    // Defensive: log and skip if assumption is violated.
                    warn!("collect_provenance_from_bundle: non-transient member encountered; skipping");
                    continue;
                }
            };

            // Read loader key (required to index provenance).
            let maybe_loader_key =
                transient_member_reference.property_value(context, CorePropertyTypeName::Key)?;
            let loader_key = match maybe_loader_key {
                Some(BaseValue::StringValue(key)) if !key.0.is_empty() => key,
                _ => continue, // no key → cannot index
            };

            // Read start byte offset (optional).
            let maybe_offset = transient_member_reference
                .property_value(context, CorePropertyTypeName::StartUtf8ByteOffset)?;
            let start_utf8_byte_offset = match maybe_offset {
                Some(BaseValue::IntegerValue(i)) => Some(i.0),
                _ => None,
            };

            // Insert first occurrence; subsequent occurrences become **ErrorWithContext**.
            provenance_index
                .entry(loader_key.clone())
                .and_modify(|existing| {
                    warn!(
                        "collect_provenance_from_bundle: duplicate loader key '{}' encountered; \
                         keeping first from file '{}', ignoring subsequent from file '{}'",
                        loader_key.0, existing.filename.0, filename.0
                    );

                    let first_offset = existing.start_utf8_byte_offset.unwrap_or(0);
                    let second_offset = start_utf8_byte_offset.unwrap_or(0);

                    let message = format!(
                        "Duplicate loader_holon key '{}' across HolonLoadSet. \
                         First occurrence: file='{}', offset={}. \
                         Subsequent occurrence: file='{}', offset={}.",
                        loader_key.0, existing.filename.0, first_offset, filename.0, second_offset,
                    );

                    // Attach the loader key so make_error_holons_best_effort can
                    // enrich with filename/offset via the provenance index.
                    let error = HolonError::DuplicateError(
                        "duplicate loader_holon key in HolonLoadSet".into(),
                        message,
                    );
                    duplicate_errors
                        .push(ErrorWithContext::new(error).with_loader_key(loader_key.clone()));
                })
                .or_insert(FileProvenance { filename: filename.clone(), start_utf8_byte_offset });
        }

        Ok(())
    }

    /// Helper to read a required string property from a holon reference.
    /// Returns `HolonError::EmptyField("<label> missing or empty")` if absent or empty.
    fn read_required_string_property(
        context: &dyn HolonsContextBehavior,
        reference: &TransientReference,
        property_name: CorePropertyTypeName,
    ) -> Result<MapString, HolonError> {
        let maybe_value = reference.property_value(context, &property_name)?;

        match maybe_value {
            Some(BaseValue::StringValue(s)) if !s.0.is_empty() => Ok(s),
            _ => Err(HolonError::EmptyField(property_name.to_property_name().to_string())),
        }
    }

    /// Construct a **transient** HolonLoadResponse:
    ///  - sets properties,
    ///  - attaches any error holons via HAS_LOAD_ERROR (declared),
    ///  - returns the *transient* response reference.
    fn build_response(
        &self,
        context: &dyn HolonsContextBehavior,
        run_id: i64, // uuid::Uuid,
        holons_staged: i64,
        holons_committed: i64,
        links_created: i64,
        errors_encountered: i64,
        total_bundles: i64,
        total_loader_holons: i64,
        summary: String,
        transient_error_references: Vec<TransientReference>,
    ) -> Result<TransientReference, HolonError> {
        debug!("Building HolonLoadResponse for run_id={}", run_id);

        // 1) Create the transient under a short-lived write lock, then DROP the lock
        let response_reference = {
            let transient_service_handle =
                context.get_space_manager().get_transient_behavior_service();
            let service = transient_service_handle
                .write()
                .map_err(|_| HolonError::FailedToBorrow("Transient service write".into()))?;
            let response_key = MapString(format!("HolonLoadResponse.{}", run_id));
            service.create_empty(response_key)?
        }; // <- write lock released here

        // Mutate the holon via its reference
        let mut response_reference = response_reference;

        // 2) Set core counters
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::HolonsStaged,
            BaseValue::IntegerValue(MapInteger(holons_staged)),
        )?;
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::HolonsCommitted,
            BaseValue::IntegerValue(MapInteger(holons_committed)),
        )?;
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::LinksCreated,
            BaseValue::IntegerValue(MapInteger(links_created)),
        )?;
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::ErrorCount,
            BaseValue::IntegerValue(MapInteger(errors_encountered)),
        )?;
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::DanceSummary,
            BaseValue::StringValue(MapString(summary)),
        )?;

        // 3) Set set-level counters
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::TotalBundles,
            BaseValue::IntegerValue(MapInteger(total_bundles)),
        )?;

        response_reference.with_property_value(
            context,
            CorePropertyTypeName::TotalLoaderHolons,
            BaseValue::IntegerValue(MapInteger(total_loader_holons)),
        )?;

        // 4) Attach any error holons
        if !transient_error_references.is_empty() {
            let error_refs: Vec<HolonReference> =
                transient_error_references.into_iter().map(HolonReference::Transient).collect();

            response_reference.add_related_holons(
                context,
                CoreRelationshipTypeName::HasLoadError,
                error_refs,
            )?;
        }

        debug!(
            "HolonLoadResponse built: staged={}, committed={}, links_created={}, errors={}",
            holons_staged, holons_committed, links_created, errors_encountered
        );

        Ok(response_reference)
    }
}
