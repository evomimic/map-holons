// crates/holons_core/src/holon_loader/errors.rs

use crate::{
    reference_layer::{TransientReference, WritableHolon},
    HolonReference, HolonsContextBehavior,
};
use base_types::{BaseValue, MapString};
use core_types::HolonError;
use std::sync::atomic::{AtomicU32, Ordering};
use type_names;
use type_names::CorePropertyTypeName::{ErrorMessage, ErrorType};
use type_names::CoreRelationshipTypeName;

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

/// Build a transient HolonError holon with {error_type, error_message} **and**
/// set its descriptor to `HolonErrorType`.
/// Caller can attach it to the response via REL_HAS_LOAD_ERROR.
pub fn make_error_holon_typed(
    context: &dyn HolonsContextBehavior,
    holon_error_type_descriptor: HolonReference, // resolved HolonErrorType descriptor
    err: &HolonError,
) -> Result<TransientReference, HolonError> {
    let transient_reference = create_empty_error_holon(context)?;
    transient_reference.with_descriptor(context, holon_error_type_descriptor)?;
    populate_error_fields(context, &transient_reference, err)?;
    Ok(transient_reference)
}

/// Build a transient HolonError holon with {error_type, error_message} **without**
/// setting any descriptor. Use when `HolonErrorType` descriptor is unavailable.
pub fn make_error_holon_untyped(
    context: &dyn HolonsContextBehavior,
    err: &HolonError,
) -> Result<TransientReference, HolonError> {
    let transient_reference = create_empty_error_holon(context)?;
    populate_error_fields(context, &transient_reference, err)?;
    Ok(transient_reference)
}

// Convenience: create & populate in one call and attach to a response holon.
pub fn attach_error_to_response(
    context: &dyn HolonsContextBehavior,
    response_holon: &TransientReference, // e.g., HolonLoadResponse
    maybe_error_type_descriptor: Option<HolonReference>,
    err: &HolonError,
) -> Result<TransientReference, HolonError> {
    let error_ref = match maybe_error_type_descriptor {
        Some(desc) => make_error_holon_typed(context, desc, err)?,
        None => make_error_holon_untyped(context, err)?,
    };

    response_holon.add_related_holons(
        context,
        CoreRelationshipTypeName::HasLoadError.as_relationship_name().clone(),
        vec![HolonReference::Transient(error_ref.clone())],
    )?;

    Ok(error_ref)
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn create_empty_error_holon(
    context: &dyn HolonsContextBehavior,
) -> Result<TransientReference, HolonError> {
    // Unique, local-only key (fast and deterministic within a process).
    let id = ERROR_SEQ.fetch_add(1, Ordering::Relaxed);
    let key = MapString(format!("loader-error-{id}"));

    // Avoid chaining borrows on temporaries; confine the Ref<'_> to a short scope.
    let transient_behavior_service_rc =
        context.get_space_manager().get_transient_behavior_service();

    let transient_reference = {
        let svc = transient_behavior_service_rc.borrow();
        svc.create_empty(key)?
    };

    Ok(transient_reference)
}

fn populate_error_fields(
    context: &dyn HolonsContextBehavior,
    error_ref: &TransientReference,
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
