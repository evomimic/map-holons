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

use base_types::{BaseValue, MapInteger, MapString};
use core_types::HolonError;

use crate::{
    // Reference-layer high-level operations
    commit_api, create_empty_transient_holon,
    // Core reference-layer traits/types
    HolonReference, HolonsContextBehavior, ReadableHolon, WritableHolon,
};
use crate::reference_layer::TransientReference;

use super::names as N;
use super::errors as E;

// Loader modules and helpers
use crate::holon_loader::errors::make_error_holon; // (context, holon_error_type_desc, &HolonError) -> TransientReference
use crate::holon_loader::loader_holon_mapper::{LoaderHolonMapper, MapperOutput};
use crate::holon_loader::loader_ref_resolver::{LoaderRefResolver, ResolverOutcome};
use crate::holon_loader::names::ERROR_TYPE_KEY;

/// HolonLoaderController: top-level coordinator for the loader pipeline.
#[derive(Debug, Default)]
pub struct HolonLoaderController {
    /// Owned mapper output for this call; holds staged refs & queued rel refs alive.
    mapper_out: Option<MapperOutput>,

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

        let mapper_out = LoaderHolonMapper::map_bundle(context, bundle)?;
        // NOTE: if/when keyless staging is supported, have the mapper return staged_all and use that length.
        self.staged_count = mapper_out.key_index.len() as i64;
        self.mapper_out = Some(mapper_out);

        // We will accumulate HolonErrors from all phases first,
        // then (only if any exist) resolve HolonErrorType and build error holons.
        let mut pending_errors: Vec<HolonError> = Vec::new();

        // Take ownership exactly once; borrow internals safely thereafter.
        let mut mo = self.mapper_out.take().expect("mapper_out set");

        // Collect **mapper**-side errors (non-fatal, per-item).
        pending_errors.extend(std::mem::take(&mut mo.errors));

        // ─────────────────────────────────────────────────────────────────────
        // PASS 2: resolve queued references (declared + inverse + DescribedBy)
        //         and write declared links against the staged holons
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_bundle - pass2_resolve");

        let ResolverOutcome {
            links_created,
            errors: resolve_errors,
        } = LoaderRefResolver::resolve_all(
            context,
            &mo.key_index,
            std::mem::take(&mut mo.queued_rel_refs),
        )?;

        // Collect **resolver**-side errors.
        pending_errors.extend(resolve_errors);

        // ─────────────────────────────────────────────────────────────────────
        // COMMIT: persist all staged holons (bulk commit)
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_bundle - commit");

        // Pre-commit count from staging behavior
        let holons_staged = context
            .get_space_manager()
            .get_staging_behavior_access()
            .borrow()
            .staged_count() as i64;

        let commit_result = commit_api(context);

        let mut holons_committed: i64 = 0;
        match commit_result {
            Ok(cr) => {
                holons_committed = cr.saved_holons.len() as i64;

                // Incomplete commit ⇒ surface as a loader error holon.
                if !cr.is_complete() {
                    pending_errors.push(HolonError::CommitFailure(
                        "Commit incomplete; some holons failed validation or persistence.".into(),
                    ));
                }
            }
            Err(e) => {
                // Entire commit failed
                holons_committed = 0;
                pending_errors.push(e);
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // BUILD RESPONSE: stage a HolonLoadResponse + attach error holons
        //                (resolve HolonErrorType descriptor only if needed)
        // ─────────────────────────────────────────────────────────────────────
        info!("HolonLoaderController::load_bundle - build_response(transient)");

        // If there are errors to emit, we must resolve the HolonErrorType descriptor.
        let mut error_refs = Vec::new();
        if !pending_errors.is_empty() {
            let error_desc = Self::resolve_holon_error_type_descriptor(context, &mo)?;
            for e in pending_errors {
                error_refs.push(make_error_holon(context, error_desc.clone(), &e)?);
            }
        }

        let status = if error_refs.is_empty() {
            MapString("OK".to_string())
        } else {
            // Align with your dancer’s response codes as needed.
            MapString("UnprocessableEntity".to_string())
        };

        let summary = if error_refs.is_empty() {
            format!(
                "{} holons staged; {} committed; {} links created.",
                holons_staged, holons_committed, links_created
            )
        } else {
            format!(
                "{} holons staged; {} committed; {} links created; {} error(s) encountered.",
                holons_staged,
                holons_committed,
                links_created,
                error_refs.len()
            )
        };

        let response = self.build_response(
            context,
            status,
            holons_staged,
            holons_committed,
            error_refs.len() as i64,
            summary,
            error_refs,
        )?;

        info!("HolonLoaderController::load_bundle - done");
        Ok(response)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Resolve the HolonErrorType descriptor `HolonReference` needed to construct error holons.
    ///
    /// Strategy:
    /// 1) Try in-bundle: look up by well-known key in the mapper's `key_index`.
    /// 2) (TODO) Try saved lookup via the space/registry if not found in the bundle.
    ///
    /// If not found, return a `HolonError` (prefer `HolonNotFound` or `InvalidType`).
    fn resolve_holon_error_type_descriptor(
        context: &dyn HolonsContextBehavior,
        mo: &MapperOutput,
    ) -> Result<HolonReference, HolonError> {
        // 1) In-bundle by key (adjust the key if your schema uses a different one).
        const ERROR_TYPE_KEY: &str = "HolonErrorType";
        if let Some(staged) = mo.key_index.get(&base_types::MapString(ERROR_TYPE_KEY.to_string())) {
            return Ok(HolonReference::Staged(staged.clone()));
        }

        // 2) TODO: Saved lookup by key or type-name (not yet implemented here).
        //    Example shape once available:
        //    if let Some(desc) = lookup_descriptor_by_key(context, MapString(ERROR_TYPE_KEY.into()))? {
        //        return Ok(desc);
        //    }

        Err(HolonError::HolonNotFound(
            "HolonErrorType descriptor not found in bundle (saved lookup not implemented)".into(),
        ))
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
        error_count: i64,
        summary: String,
        mut error_holons: Vec<TransientReference>,
    ) -> Result<TransientReference, HolonError> {
        // Build response as a transient with properties…
        let rsp = create_empty_transient_holon(
            context,
            MapString("CoreLoaderControllerResponse".to_string()),
        )?;

        rsp.with_property_value(
            context,
            N::prop(N::PROP_RESPONSE_STATUS_CODE),
            BaseValue::StringValue(response_status_code),
        )?;
        rsp.with_property_value(
            context,
            N::prop(N::PROP_HOLONS_STAGED),
            BaseValue::IntegerValue(MapInteger(holons_staged)),
        )?;
        rsp.with_property_value(
            context,
            N::prop(N::PROP_HOLONS_COMMITTED),
            BaseValue::IntegerValue(MapInteger(holons_committed)),
        )?;
        rsp.with_property_value(
            context,
            N::prop(N::PROP_ERROR_COUNT),
            BaseValue::IntegerValue(MapInteger(error_count)),
        )?;
        rsp.with_property_value(
            context,
            N::prop(N::PROP_DANCE_SUMMARY),
            BaseValue::StringValue(MapString(summary)),
        )?;

        // Attach errors to the response (declared link), if any.
        if !error_holons.is_empty() {
            rsp.add_related_holons(
                context,
                N::rel(N::REL_HAS_LOAD_ERROR),
                error_holons
                    .drain(..)
                    .map(|tr| HolonReference::Transient(tr))
                    .collect::<Vec<_>>(),
            )?;
        }

        Ok(rsp)
    }
}
