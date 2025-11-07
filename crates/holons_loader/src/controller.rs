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

use tracing::{debug, info};
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
        Self::default()
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
        info!("HolonLoaderController::load_bundle - starting");
        let run_id = 1; // Temporary fixed run_id until we wire in Uuid

        // ─────────────────────────────────────────────────────────────────────
        // PASS 1: map & stage node holons (properties only); queue relationship refs
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_bundle - pass1_stage");

        let mut mapper_output = LoaderHolonMapper::map_bundle(context, bundle)?;
        // For now we approximate staged_count by the number of staged targets created in Pass 1.
        // (If/when keyless or extra targets appear, have the mapper return exact staged_count.)
        let staged_count = mapper_output.staged_count;

        // If Pass 1 produced any errors or the bundle was empty,
        // build the response now and return (skip Pass 2 & commit).
        let mapper_errors = std::mem::take(&mut mapper_output.errors);
        let is_empty_bundle =
            staged_count == 0 && mapper_output.queued_relationship_references.is_empty();
        if !mapper_errors.is_empty() || is_empty_bundle {
            info!(
                "HolonLoaderController::load_bundle - early return: {}",
                if !mapper_errors.is_empty() { "pass1 errors" } else { "empty bundle" }
            );

            // Build error holons (prefer typed; fallback to untyped if descriptor missing)
            let error_holons = make_error_holons_best_effort(context, &mapper_errors)
                .unwrap_or_else(|e| {
                    info!("Failed to build error holons (pass1); proceeding without: {}", e);
                    Vec::new()
                });

            let summary = if !mapper_errors.is_empty() {
                format!(
                    "Pass 1 reported {} error(s). Pass 2 and commit were skipped.",
                    error_holons.len()
                )
            } else {
                "Empty bundle: no LoaderHolons found; nothing to process.".into()
            };

            let response_reference = self.build_response(
                context,
                run_id,
                staged_count,
                0,
                0,
                error_holons.len() as i64,
                summary,
                error_holons,
            )?;

            info!("HolonLoaderController::load_bundle - done (pass1 short-circuit)");
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
            info!("HolonLoaderController::load_bundle - pass2 errors, short-circuit before commit");

            let error_holons = make_error_holons_best_effort(context, &resolver_errors)
                .unwrap_or_else(|e| {
                    info!("Failed to build error holons (pass2); proceeding without: {}", e);
                    Vec::new()
                });

            let response_reference = self.build_response(
                context,
                run_id,
                staged_count,
                0,
                links_created,
                error_holons.len() as i64,
                format!(
                    "Pass 2 reported {} error(s). Commit was skipped. {} holons staged; 0 committed; {} links attempted.",
                    error_holons.len(), staged_count, links_created
                ),
                error_holons,
            )?;

            info!("HolonLoaderController::load_bundle - done (pass2 short-circuit)");
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
            0,
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

        info!("HolonLoaderController::load_bundle - done");
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
        use tracing::info;

        info!("Building HolonLoadResponse for run_id={}", run_id);

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

        // We'll mutate the holon via its reference
        let mut response_reference = response_reference;

        // Temporary helper to log a property read-back (kept inside this function)
        fn log_read_back(
            ctx: &dyn HolonsContextBehavior,
            r: &TransientReference,
            label: &str,
            pname: &PropertyName,
        ) {
            match r.property_value(ctx, pname) {
                Ok(Some(v)) => info!("READ-BACK {label} -> {:?}", v),
                Ok(None) => info!("READ-BACK {label} -> None"),
                Err(e) => info!("READ-BACK {label} -> ERROR: {e:?}"),
            }
        }

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
