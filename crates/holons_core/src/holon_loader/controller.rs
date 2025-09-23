// crates/holons_core/src/holon_loader/controller.rs
//
// Orchestrates the two-pass holon loading flow:
//
//   Pass 1: Map & stage node holons (properties only); queue relationship references.
//   Pass 2: Resolve queued edges to concrete declared links (declared, inverse, DescribedBy).
//   Commit: Persist staged holons in one bulk commit.
//   Respond: Return a *staged* HolonLoadResponse (with related *staged* HolonLoadError holons).
//
// This controller keeps only per-call, in-memory state (no cross-call persistence).
// It is intentionally thin: it wires together Mapper → Resolver → Commit → Response.

use std::collections::HashMap;

use tracing::info;

use base_types::{BaseValue, MapInteger, MapString};
use core_types::HolonError;

use crate::{commit_api, stage_new_holon_api};

use crate::{
    // Re-exported from holons_core::lib
    CommitResponse, HolonReference, HolonsContextBehavior, StagedReference,
    core_shared_objects::holon::TransientHolon,
    // Public loader modules (re-exported by holon_loader::mod)
    holon_loader::loader_holon_mapper::{LoaderHolonMapper, MapperOutput},
    holon_loader::loader_ref_resolver::{LoaderRefResolver, ResolverOutcome},
    // Private loader modules (mod.rs pub(crate) re-exports)
    holon_loader::errors::{build_load_error_holon, map_holon_error},
};
use crate::reference_layer::WriteableHolonReferenceLayer;
use super::names as N;

/// HolonLoaderController: top-level coordinator for the loader pipeline.
#[derive(Debug, Default)]
pub struct HolonLoaderController {
    /// Fast local resolution: (key → staged holon reference) for this load call.
    key_index: HashMap<MapString, StagedReference>,
    /// The unresolved edge descriptors (LoaderRelationshipReference holons) collected in Pass 1.
    queued_rel_refs: Vec<TransientHolon>,  // any reason to make this a HashSet to avoid duplicates?
}

impl HolonLoaderController {
    /// Create a new controller with empty per-call caches.
    pub fn new() -> Self {
        Self::default()
    }


    /// Entry point invoked by the dance adapter.
    ///
    /// `segment` must be a raw `TransientHolon` typed as `HolonLoaderSegmentType`.
    ///
    /// Returns a **staged** `HolonLoadResponse` (with related **staged** `HolonLoadError` holons).
    /// Caller may inspect as needed.
    pub fn load_segment(
        &mut self,
        context: &dyn HolonsContextBehavior,
        segment: TransientHolon,
    ) -> Result<StagedReference, HolonError> {
        info!("HolonLoaderController::load_segment - start");

        // ─────────────────────────────────────────────────────────────────────
        // Extract loader holons (segment --HOLONS_TO_LOAD--> LoaderHolon*)
        // ─────────────────────────────────────────────────────────────────────
        let loader_holon_refs = self.extract_loader_holon_refs(&segment)?;

        // ─────────────────────────────────────────────────────────────────────
        // PASS 1: map & stage node holons (properties only); queue relationship refs
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_segment - pass1_stage");

        let MapperOutput {
            keyed_staged,
            queued_rel_refs,
        } = LoaderHolonMapper::map_and_stage(context, &loader_holon_refs)?;

        // Populate the fast (key → staged_ref) index for resolver lookups.
        self.key_index.clear();
        for (k, r) in keyed_staged {
            self.key_index.insert(k, r);
        }
        // Hand off queued edge descriptors to Pass 2.
        self.queued_rel_refs = queued_rel_refs;

        // ─────────────────────────────────────────────────────────────────────
        // PASS 2: resolve queued references (declared + inverse + DescribedBy)
        //         and write declared links against the staged holons
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_segment - pass2_resolve");

        let ResolverOutcome {
            links_created,
            errors: mut_resolve_errs,
        } = LoaderRefResolver::resolve_all(
            context,
            &self.key_index,
            std::mem::take(&mut self.queued_rel_refs),
        )?;

        // ─────────────────────────────────────────────────────────────────────
        // COMMIT: persist all staged holons (bulk commit)
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_segment - commit");

        // pre-commit count from staging area
        let holons_staged = context
            .get_space_manager()
            .get_staging_behavior_access()
            .borrow()
            .staged_count();
        let mut holons_committed: i64 = 0;  // authoritative post-commit count comes from CommitResponse

        // We prefer to *report* errors in the response rather than failing early,
        // so we collect errors here and continue to build the response.
        let commit_result: Result<CommitResponse, HolonError> = commit_api(context);

        // Collect loader-facing error holons built from internal HolonErrors.
        let mut error_transients: Vec<TransientHolon> = Vec::new();

        // Map *resolver* errors to loader error holons.
        for e in mut_resolve_errs {
            error_transients.push(build_load_error_holon(map_holon_error(e)));
        }

        match commit_result {
            Ok(cr) => {
                // Count of committed holons for the loader response.
                holons_committed = cr.saved_holons.len() as i64;

                // If the commit was incomplete, there may be per-holon errors embedded
                // in staged holons that failed. Surface a generic CommitFailure here;
                // finer-grained mapping can be added later if desired.
                if !cr.is_complete() {
                    error_transients.push(build_load_error_holon(map_holon_error(
                        HolonError::CommitFailure("Commit incomplete; some holons failed validation or persistence.".into())
                    )));
                }
            }
            Err(e) => {
                // Commit failed entirely; report the error and provide a staged count.
                holons_committed = 0;

                error_transients.push(build_load_error_holon(map_holon_error(e)));
            }
        }

        // Do we want to empty the nursery here before staging response/errors?

        // ─────────────────────────────────────────────────────────────────────
        // BUILD RESPONSE: stage a HolonLoadResponse + stage & attach error holons
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_segment - build_response(staged)");

        // Basic status + summary logic. We could get fancier later.
        let (status, summary) = if error_transients.is_empty() {
            (
                MapString("OK".to_string()),
                format!(
                    "{} holons staged; {} committed; {} links created.",
                    holons_staged, holons_committed, links_created
                ),
            )
        } else {
            (
                MapString("UnprocessableEntity".to_string()),
                format!(
                    "{} holons staged; {} committed; {} links created; {} error(s) encountered.",
                    holons_staged,
                    holons_committed,
                    links_created,
                    error_transients.len()
                ),
            )
        };

        let response_staged = self.build_response_staged(
            context,
            status,
            holons_staged,
            holons_committed,
            error_transients.len() as i64,
            summary,
            error_transients,
        )?;

        info!("HolonLoaderController::load_segment - done");
        Ok(response_staged)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Pull `LoaderHolon` references off the segment via `HOLONS_TO_LOAD`.
    ///
    /// This uses relationship traversal rather than inspecting properties so the
    /// controller remains schema-driven and decoupled from field details.
    fn extract_loader_holon_refs(
        &self,
        segment: &TransientHolon,
    ) -> Result<Vec<HolonReference>, HolonError> {
        // For TransientHolon, relationship traversal is available without a context.
        let loader_holons = segment.get_related_holons(&N::rel(N::REL_HOLONS_TO_LOAD))?;
        Ok(loader_holons.get_members().clone())
    }

    /// Construct a **staged** HolonLoadResponse:
    ///  - sets properties,
    ///  - stages each HolonLoadError transient,
    ///  - attaches them via HAS_LOAD_ERROR (declared),
    ///  - returns the staged response.
    fn build_response_staged(
        &self,
        context: &dyn HolonsContextBehavior,
        response_status_code: MapString,
        holons_staged: i64,
        holons_committed: i64,
        errors_encountered: i64,
        summary: String,
        error_holons: Vec<TransientHolon>,
    ) -> Result<StagedReference, HolonError> {
        // Build response as a transient with properties…
        let mut resp_t = TransientHolon::new();

        let _ = resp_t.with_property_value(
            N::prop(N::PROP_RESPONSE_STATUS_CODE),
            Some(BaseValue::StringValue(response_status_code)),
        );
        let _ = resp_t.with_property_value(
            N::prop(N::PROP_HOLONS_STAGED),
            Some(BaseValue::IntegerValue(MapInteger(holons_staged))),
        );
        let _ = resp_t.with_property_value(
            N::prop(N::PROP_HOLONS_COMMITTED),
            Some(BaseValue::IntegerValue(MapInteger(holons_committed))),
        );
        let _ = resp_t.with_property_value(
            N::prop(N::PROP_ERRORS_ENCOUNTERED),
            Some(BaseValue::IntegerValue(MapInteger(errors_encountered))),
        );
        let _ = resp_t.with_property_value(
            N::prop(N::PROP_SUMMARY),
            Some(BaseValue::StringValue(MapString(summary))),
        );

        // …then stage it to get a writable staged reference.
        let response_s = stage_new_holon_api(context, resp_t)?;

        // Stage each error transient and collect as HolonReferences::Staged
        if !error_holons.is_empty() {
            let rel = N::rel(N::REL_HAS_LOAD_ERROR);

            let mut err_refs: Vec<HolonReference> = Vec::with_capacity(error_holons.len());
            for e in error_holons {
                let s = stage_new_holon_api(context, e)?;
                err_refs.push(HolonReference::from_staged(s));
            }

            // Attach staged errors to the staged response (declared link)
            response_s.add_related_holons_ref_layer(context, rel, err_refs)?;
        }

        Ok(response_s)
    }
}