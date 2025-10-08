// crates/holons_core/src/holon_loader/controller.rs
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

use tracing::info;

use crate::reference_layer::TransientReference;
use crate::{
    // Reference-layer high-level operations
    commit_api,
    create_empty_transient_holon,
    // Core reference-layer traits/types
    HolonReference,
    HolonsContextBehavior,
    ReadableHolon,
    WritableHolon,
};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::HolonError;
use type_names;
use type_names::CorePropertyTypeName::{
    DanceSummary, ErrorCount, HolonsCommitted, HolonsStaged, ResponseStatusCode,
};
use type_names::CoreRelationshipTypeName::HasLoadError;

// Loader modules and helpers
use super::errors as E;
use crate::holon_loader::loader_holon_mapper::{LoaderHolonMapper, MapperOutput};
use crate::holon_loader::loader_ref_resolver::{LoaderRefResolver, ResolverOutcome};

const ERROR_TYPE_KEY: &str = "HolonErrorType";

/// HolonLoaderController: top-level coordinator for the loader pipeline.
#[derive(Debug, Default)]
pub struct HolonLoaderController {
    /// Stats for response construction (purely informational).
    staged_count: i64,
    committed_count: i64,
    error_count: i64,
}

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
        info!("HolonLoaderController::load_bundle - start");

        // ─────────────────────────────────────────────────────────────────────
        // PASS 1: map & stage node holons (properties only); queue relationship refs
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_bundle - pass1_stage");

        let mut mapper_output = LoaderHolonMapper::map_bundle(context, bundle)?;
        // For now we approximate staged_count by the number of staged targets created in Pass 1.
        // (If/when keyless or extra targets appear, have the mapper return exact staged_count.)
        self.staged_count = mapper_output.staged_count as i64;

        // If Pass 1 produced any errors, build the response now and return (skip Pass 2 & commit).
        let mapper_errors = std::mem::take(&mut mapper_output.errors);
        if !mapper_errors.is_empty() {
            info!("HolonLoaderController::load_bundle - pass1 errors, short-circuit before pass2/commit");

            // Build error holons (prefer typed; fallback to untyped if descriptor missing)
            let error_holons = self.build_error_holons_best_effort(context, &mapper_errors)?;

            let response_reference = self.build_response(
                context,
                MapString("UnprocessableEntity".into()),
                self.staged_count,
                0,
                error_holons.len() as i64,
                format!(
                    "Pass 1 reported {} error(s). Pass 2 and commit were skipped.",
                    error_holons.len()
                ),
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

        let ResolverOutcome { links_created, errors: mut resolver_errors } =
            LoaderRefResolver::resolve_relationships(context, queued_relationship_references)?;

        // If Pass 2 produced any errors, build the response now and return (skip commit).
        if !resolver_errors.is_empty() {
            info!("HolonLoaderController::load_bundle - pass2 errors, short-circuit before commit");

            let error_holons = self.build_error_holons_best_effort(context, &resolver_errors)?;

            let response_reference = self.build_response(
                context,
                MapString("UnprocessableEntity".into()),
                self.staged_count,
                0,
                error_holons.len() as i64,
                format!(
                    "Pass 2 reported {} error(s). Commit was skipped. {} holons staged; 0 committed; {} links attempted.",
                    error_holons.len(), self.staged_count, links_created
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

        let commit_response = commit_api(context)?;
        // Basic accounting per meeting notes:
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
            MapString(if commit_ok { "OK" } else { "Accepted" }.into()),
            self.staged_count,
            holons_committed,
            0,
            if commit_ok {
                format!(
                    "{} holons staged; {} committed; {} abandoned; {} attempts.",
                    self.staged_count, holons_committed, holons_abandoned, commits_attempted
                )
            } else {
                format!(
                    "{} holons staged; {} committed; {} abandoned; {} attempts; commit incomplete.",
                    self.staged_count, holons_committed, holons_abandoned, commits_attempted
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

    /// Build loader-facing error holons for a list of HolonErrors.
    /// Strategy:
    ///   1) Try to resolve HolonErrorType and emit **typed** error holons.
    ///   2) If resolution fails, emit **untyped** error holons (no descriptor) with {error_type, error_message}.
    fn build_error_holons_best_effort(
        &self,
        context: &dyn HolonsContextBehavior,
        errors: &[HolonError],
    ) -> Result<Vec<TransientReference>, HolonError> {
        // Try to resolve the HolonErrorType descriptor (by key or query).
        if let Ok(holon_error_type_descriptor) = resolve_holon_error_type_descriptor(context) {
            let mut out = Vec::with_capacity(errors.len());
            for err in errors {
                // Use your existing helper which sets {error_type, error_message} and the descriptor:
                out.push(E::make_error_holon_typed(
                    context,
                    holon_error_type_descriptor.clone(),
                    err,
                )?);
            }
            return Ok(out);
        }

        // Fallback: emit untyped error holons (no descriptor), still include fields.
        let mut out = Vec::with_capacity(errors.len());
        for err in errors {
            out.push(E::make_error_holon_untyped(context, err)?);
        }
        Ok(out)
    }

    /// Construct a **transient** HolonLoadResponse:
    ///  - sets properties,
    ///  - attaches any error holons via HAS_LOAD_ERROR (declared),
    ///  - returns the *transient* response reference.
    fn build_response(
        &self,
        context: &dyn HolonsContextBehavior,
        response_status_code: MapString,
        holons_staged: i64,
        holons_committed: i64,
        errors_encountered: i64,
        summary: String,
        transient_error_references: Vec<TransientReference>,
    ) -> Result<TransientReference, HolonError> {
        // Build response as a transient with properties…
        let response_reference = create_empty_transient_holon(
            context,
            MapString("CoreLoaderControllerResponse".to_string()),
        )?;

        response_reference.with_property_value(
            context,
            ResponseStatusCode.as_property_name(),
            BaseValue::StringValue(response_status_code),
        )?;
        response_reference.with_property_value(
            context,
            HolonsStaged.as_property_name(),
            BaseValue::IntegerValue(MapInteger(holons_staged)),
        )?;
        response_reference.with_property_value(
            context,
            HolonsCommitted.as_property_name(),
            BaseValue::IntegerValue(MapInteger(holons_committed)),
        )?;
        response_reference.with_property_value(
            context,
            ErrorCount.as_property_name(),
            BaseValue::IntegerValue(MapInteger(errors_encountered)),
        )?;
        response_reference.with_property_value(
            context,
            DanceSummary.as_property_name(),
            BaseValue::StringValue(MapString(summary)),
        )?;

        // attach any error holons via HAS_LOAD_ERROR
        if !transient_error_references.is_empty() {
            let error_holon_references: Vec<HolonReference> =
                transient_error_references.into_iter().map(HolonReference::Transient).collect();

            response_reference.add_related_holons(
                context,
                HasLoadError.as_relationship_name().clone(),
                error_holon_references,
            )?;
        }

        Ok(response_reference)
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Descriptor resolution (best-effort): prefer key lookup, fallback to query.
// Keep tiny and side effect free; only used when we have errors to emit.
// ─────────────────────────────────────────────────────────────────────────

fn resolve_holon_error_type_descriptor(
    _context: &dyn HolonsContextBehavior,
) -> Result<HolonReference, HolonError> {
    // TODO: real lookup (by known key or by type name).
    // For now, we return an error to exercise the untyped fallback.
    Err(HolonError::HolonNotFound("HolonErrorType descriptor not found".into()))
}
