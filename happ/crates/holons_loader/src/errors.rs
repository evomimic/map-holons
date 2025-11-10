// shared_crates/holons_loader/src/errors.rs

use holons_prelude::prelude::CorePropertyTypeName::{ErrorMessage, ErrorType};
use holons_prelude::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{error, warn};

// Global counter for generating unique error holon keys
static ERROR_SEQ: AtomicU32 = AtomicU32::new(1);

/// Map HolonError -> stable snake_case code for response analytics/UI.
pub fn error_type_code(err: &HolonError) -> &'static str {
    use HolonError::*;
    match err {
        // Likely along the loader path:
        DuplicateError(_, _) => "duplicate",
        EmptyField(_) => "empty_field",
        InvalidRelationship(_, _) => "invalid_relationship",
        InvalidParameter(_) => "invalid_parameter",
        ValidationError(_) => "validation_error",
        CommitFailure(_) => "commit_failure",
        UnexpectedValueType(_, _) => "unexpected_value_type",
        InvalidType(_) => "invalid_type",
        HolonNotFound(_) => "holon_not_found",
        InvalidHolonReference(_) => "invalid_holon_reference",
        NotAccessible(_, _) => "not_accessible",

        // Group the rest under a generic bucket to avoid leaking internals
        _ => "server_error",
    }
}

/// Build transient HolonErrorType holons for reporting load errors.
pub fn make_error_holons_best_effort(
    context: &dyn HolonsContextBehavior,
    errors: &[HolonError],
) -> Result<Vec<TransientReference>, HolonError> {
    if errors.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity(errors.len());
    // Try resolving the HolonErrorType descriptor once.
    let holon_error_type_descriptor = resolve_holon_error_type_descriptor(context).ok();

    for load_error in errors {
        match make_error_holon(context, holon_error_type_descriptor.clone(), load_error) {
            Ok(transient_reference) => out.push(transient_reference),
            Err(e) => {
                // This indicates a system-level issue (e.g., failed to allocate transient holon).
                // There’s no reliable way to continue building error holons.
                error!("failed to create error holon (typed or untyped): {}", e);
                return Err(e);
            }
        }
    }

    Ok(out)
}

/// Builds a transient **HolonError** holon representing the specified `HolonError`.
///
/// This function creates a new transient holon and populates it with the
/// standard error fields (e.g., `error_type`, `error_message`).
/// If a `descriptor` is provided, it is attached via `with_descriptor()` to identify
/// the holon's type (typically `HolonErrorType`). If `descriptor` is `None`, the holon
/// is left untyped but still contains all relevant error details.
///
/// # Arguments
/// - `context`: The active holon execution context used to access transient behavior services.
/// - `descriptor`: An optional reference to the `HolonErrorType` descriptor holon.
/// - `err`: The `HolonError` instance to encode into the transient holon.
///
/// # Returns
/// - `Ok(TransientReference)` — reference to the newly created transient error holon.
/// - `Err(HolonError)` — if the transient holon could not be created or populated.
///
/// # Behavior
/// - Always calls `create_empty_error_holon()` to allocate a new transient holon.
/// - Applies `with_descriptor()` only if a descriptor is provided.
/// - Uses `populate_error_fields()` to fill in diagnostic fields.
///
/// Use this helper to create both typed and untyped error holons from a single entry point.
pub fn make_error_holon(
    context: &dyn HolonsContextBehavior,
    descriptor: Option<HolonReference>,
    err: &HolonError,
) -> Result<TransientReference, HolonError> {
    let mut transient_reference = create_empty_error_holon(context)?;
    if let Some(desc) = descriptor {
        transient_reference.with_descriptor(context, desc)?;
    }
    populate_error_fields(context, &mut transient_reference, err)?;
    Ok(transient_reference)
}
// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn create_empty_error_holon(
    context: &dyn HolonsContextBehavior,
) -> Result<TransientReference, HolonError> {
    // Generate a unique, local-only key (fast and deterministic within a process).
    let id = ERROR_SEQ.fetch_add(1, Ordering::Relaxed);
    let key = MapString(format!("loader-error-{id}"));

    // Obtain a handle to the TransientHolonBehavior service from the Space Manager.
    let transient_behavior_service_handle =
        context.get_space_manager().get_transient_behavior_service();

    // Acquire a write lock for mutable access to the TransientHolonBehavior service.
    let transient_behavior_service = transient_behavior_service_handle.write().map_err(|_| {
        HolonError::FailedToBorrow("TransientHolonBehavior RwLock was poisoned".into())
    })?;

    // Create a new, empty transient holon using the generated key.
    let transient_reference = transient_behavior_service.create_empty(key)?;

    Ok(transient_reference)
}

fn populate_error_fields(
    context: &dyn HolonsContextBehavior,
    error_ref: &mut TransientReference,
    err: &HolonError,
) -> Result<(), HolonError> {
    let error_type: &str = error_type_code(err);

    error_ref.with_property_value(
        context,
        ErrorType.as_property_name(),
        BaseValue::StringValue(MapString(error_type.to_string())),
    )?;

    error_ref.with_property_value(
        context,
        ErrorMessage.as_property_name(),
        BaseValue::StringValue(MapString(err.to_string())),
    )?;

    Ok(())
}

/// Descriptor resolution (best-effort):
/// 1) Staged (Nursery) lookup by key "HolonErrorType"
/// 2) Saved fallback via get_all_holons() + get_by_key()
fn resolve_holon_error_type_descriptor(
    context: &dyn HolonsContextBehavior,
) -> Result<HolonReference, HolonError> {
    // Canonical key from the enum (=> "HolonErrorType")
    let key = CoreHolonTypeName::HolonErrorType.as_holon_name();

    // 1) Prefer staged (Nursery) by base key
    let staged_matches = {
        let staging_handle = context.get_space_manager().get_staging_service();

        // We only need read access to query staged holons.
        let staging_guard = staging_handle
            .read()
            .map_err(|_| HolonError::FailedToBorrow("Staging service lock poisoned".into()))?;

        // Query staged holons by base key.
        // This returns a Vec<StagedReference> without mutating state.
        staging_guard.get_staged_holons_by_base_key(&key)?
    };

    match staged_matches.len() {
        1 => {
            let staged_ref = staged_matches.into_iter().next().unwrap();
            return Ok(HolonReference::Staged(staged_ref));
        }
        n if n > 1 => {
            return Err(HolonError::DuplicateError(
                "HolonErrorType descriptor (staged)".into(),
                n.to_string(),
            ));
        }
        _ => { /* fall through to saved fallback */ }
    }

    // 2) Saved fallback: single pass over the saved index by key
    let saved_collection = get_all_holons(context)?;
    match saved_collection.get_by_key(&key) {
        Ok(Some(reference)) => Ok(reference),
        Ok(None) => Err(HolonError::HolonNotFound(format!(
            "HolonErrorType descriptor not found by key '{}'",
            key.0
        ))),
        Err(e) => Err(e),
    }
}
