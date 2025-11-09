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

use tracing::{debug, info, warn};
// use uuid::Uuid;

use holons_prelude::prelude::*;

use crate::errors::make_error_holons_best_effort;
use crate::{LoaderHolonMapper, LoaderRefResolver, ResolverOutcome};

pub const CRATE_LINK: &str = "I like loading holons with holons_loader!"; // temporary const to link crate to test crate

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
    /// - `bundle`: a *transient* HolonLoaderBundle with BUNDLE_MEMBERS → LoaderHolons
    ///
    /// Output:
    /// - `Ok(TransientReference)` to a *transient* HolonLoadResponse (message-only)
    /// - `Err(HolonError)` for system-level failures preventing any meaningful response
    pub fn load_bundle(
        &mut self,
        context: &dyn HolonsContextBehavior,
        bundle: TransientReference, // -> HolonLoaderBundle
    ) -> Result<TransientReference, HolonError> {
        // let run_id = Uuid::new_v4();
        // info!("HolonLoaderController::load_bundle - start run_id={run_id}");
        let run_id = 1; // Temporary fixed run_id until we wire in Uuid

        // ─────────────────────────────────────────────────────────────────────
        // PASS 1: map & stage node holons (properties only); queue relationship refs
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_bundle - pass1_stage");

        let mut mapper_output = LoaderHolonMapper::map_bundle(context, bundle)?;
        // For now we approximate staged_count by the number of staged targets created in Pass 1.
        // (If/when keyless or extra targets appear, have the mapper return exact staged_count.)
        let staged_count = mapper_output.staged_count;

        // Extract Pass-1 errors
        let mapper_errors = std::mem::take(&mut mapper_output.errors);

        // ─────────────────────────────────────────────────────────────────────
        // PASS 1: handle early-exit conditions (errors or empty bundle)
        // ─────────────────────────────────────────────────────────────────────

        let error_count = mapper_errors.len() as i64;
        let is_error_case = error_count > 0;

        // ─────────────────────────────────────────────────────────────────────
        // CASE 1: Pass-1 errors detected
        // ─────────────────────────────────────────────────────────────────────
        if is_error_case {
            warn!(
            "HolonLoaderController::load_bundle - early return due to Pass 1 errors ({} detected)",
            error_count
        );

            // Prefer typed error holons; bubble up if system-level failure
            let error_holons = make_error_holons_best_effort(context, &mapper_errors)?;

            let summary = format!(
                "Pass 1 reported {} error(s). Pass 2 and commit were skipped.",
                error_count
            );

            let response_reference = self.build_response(
                context,
                run_id,
                staged_count,
                0,           // holons_committed
                0,           // links_created
                error_count, // always use real error count, not holon count
                summary,
                error_holons,
            )?;

            warn!("HolonLoaderController::load_bundle - done (aborted after Pass 1)");
            return Ok(response_reference);
        }

        // ─────────────────────────────────────────────────────────────────────
        // CASE 2: Empty bundle (no holons and no relationships)
        // ─────────────────────────────────────────────────────────────────────
        // Clarify “empty bundle” semantics for readability and extensibility
        let no_staged_holons = staged_count == 0;
        let no_relationships = mapper_output.queued_relationship_references.is_empty();
        let is_empty_bundle = no_staged_holons && no_relationships;

        if is_empty_bundle {
            info!(
            "HolonLoaderController::load_bundle - early return (empty bundle: no holons, no relationships)"
        );

            let summary =
                "Empty bundle: no LoaderHolons or relationship references found; nothing to process."
                    .to_string();

            let response_reference = self.build_response(
                context,
                run_id,
                staged_count,
                0, // holons_committed
                0, // links_created
                0, // errors_encountered
                summary,
                Vec::new(), // no error holons
            )?;

            info!("HolonLoaderController::load_bundle - done (empty bundle short-circuit)");
            return Ok(response_reference);
        }

        // ─────────────────────────────────────────────────────────────────────
        // PASS 2: resolve queued references and write declared links
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_bundle - pass2_resolve");

        // Take ownership of the queued relationship references (drain from mapper_output).
        let queued_relationship_references =
            std::mem::take(&mut mapper_output.queued_relationship_references);

        let ResolverOutcome { links_created, errors: resolver_errors } =
            LoaderRefResolver::resolve_relationships(context, queued_relationship_references)?;

        // If Pass 2 produced any errors, build the response now and return (skip commit).
        if !resolver_errors.is_empty() {
            let resolver_error_count = resolver_errors.len() as i64;

            warn!(
            "HolonLoaderController::load_bundle - pass2 errors ({}), short-circuit before commit",
            resolver_error_count
        );

            let error_holons = make_error_holons_best_effort(context, &resolver_errors)
                .unwrap_or_else(|e| {
                    warn!("Failed to build error holons (pass2); proceeding without: {}", e);
                    Vec::new()
                });

            let response_reference = self.build_response(
                context,
                run_id,
                staged_count,
                0, // holons_committed
                links_created,
                resolver_error_count,
                format!(
                    "Pass 2 reported {} error(s). Commit was skipped. {} holons staged; 0 committed; {} links attempted.",
                    resolver_error_count, staged_count, links_created
                ),
                error_holons,
            )?;

            warn!("HolonLoaderController::load_bundle - done (pass2 short-circuit)");
            return Ok(response_reference);
        }

        // ─────────────────────────────────────────────────────────────────────
        // COMMIT: persist all staged holons (only if both phases succeeded)
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_bundle - commit");

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

        // We’re not surfacing per-item commit errors yet; just report via summary.
        let response_reference = self.build_response(
            context,
            run_id,
            staged_count,
            holons_committed,
            links_created,
            0, // errors_encountered
            if commit_ok {
                format!(
                    "{} holons staged; {} committed; {} abandoned; {} attempts.",
                    staged_count, holons_committed, holons_abandoned, commits_attempted
                )
            } else {
                format!(
                    "{} holons staged; {} committed; {} abandoned; {} attempts; commit incomplete.",
                    staged_count, holons_committed, holons_abandoned, commits_attempted
                )
            },
            Vec::new(),
        )?;

        debug!("HolonLoaderController::load_bundle - done");
        Ok(response_reference)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

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

        // 2) Set properties
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::HolonsStaged.as_property_name(),
            BaseValue::IntegerValue(MapInteger(holons_staged)),
        )?;
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::HolonsCommitted.as_property_name(),
            BaseValue::IntegerValue(MapInteger(holons_committed)),
        )?;
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::LinksCreated.as_property_name(),
            BaseValue::IntegerValue(MapInteger(links_created)),
        )?;
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::ErrorCount.as_property_name(),
            BaseValue::IntegerValue(MapInteger(errors_encountered)),
        )?;
        response_reference.with_property_value(
            context,
            CorePropertyTypeName::DanceSummary.as_property_name(),
            BaseValue::StringValue(MapString(summary)),
        )?;

        // 3) Attach any error holons
        if !transient_error_references.is_empty() {
            let error_refs: Vec<HolonReference> =
                transient_error_references.into_iter().map(HolonReference::Transient).collect();

            response_reference.add_related_holons(
                context,
                CoreRelationshipTypeName::HasLoadError.as_relationship_name().clone(),
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
