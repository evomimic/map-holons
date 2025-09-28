// crates/holons_core/src/holon_loader/errors.rs

use std::sync::atomic::{AtomicU32, Ordering};
use base_types::{BaseValue, MapString};
use core_types::HolonError;

use crate::{
    HolonReference, HolonsContextBehavior,
    reference_layer::{TransientReference, TransientHolonBehavior, WritableHolon},
};

use super::names as N;

// Global counter for generating unique error holon keys
static ERROR_SEQ: AtomicU32 = AtomicU32::new(1);

/// Map HolonError -> stable snake_case code for response analytics/UI.
pub fn error_type_code(err: &HolonError) -> &'static str {
    use HolonError::*;
    match err {
        // Likely along the loader path:
        DuplicateError(_, _)         => "duplicate",
        EmptyField(_)                => "empty_field",
        InvalidRelationship(_, _)    => "invalid_relationship",
        InvalidParameter(_)          => "invalid_parameter",
        ValidationError(_)           => "validation_error",
        CommitFailure(_)             => "commit_failure",
        UnexpectedValueType(_, _)    => "unexpected_value_type",
        InvalidType(_)               => "invalid_type",
        HolonNotFound(_)             => "holon_not_found",
        InvalidHolonReference(_)     => "invalid_holon_reference",
        NotAccessible(_, _)          => "not_accessible",

        // Group the rest under a generic bucket to avoid leaking internals
        _                            => "server_error",
    }
}

/// Build a transient HolonError holon with {error_type, error_message}.
/// Caller should then attach it to the response via REL_HAS_LOAD_ERROR.
pub fn make_error_holon(
    context: &dyn HolonsContextBehavior,
    holon_error_type_desc: HolonReference, // resolved HolonErrorType descriptor
    err: &HolonError,
) -> Result<TransientReference, HolonError> {
    // 1) Create a unique, local-only key
    let id = ERROR_SEQ.fetch_add(1, Ordering::Relaxed);
    let key = MapString(format!("loader-error-{id}"));

    // 2) Create empty transient holon via manager
    let tmgr_rc = context
        .get_space_manager()
        .get_transient_behavior_service();

    let transient = {
        let tmgr_ref = tmgr_rc.borrow();
        tmgr_ref.create_empty(key)?
    };

    // 3) Set its descriptor to HolonErrorType
    transient.with_descriptor(context, holon_error_type_desc)?;

    // 4) Populate properties
    let etype = error_type_code(err);
    transient.with_property_value(
        context,
        N::prop(N::PROP_ERROR_TYPE),
        BaseValue::StringValue(MapString(etype.to_string())),
    )?;

    transient.with_property_value(
        context,
        N::prop(N::PROP_ERROR_MESSAGE),
        BaseValue::StringValue(MapString(err.to_string())),
    )?;

    Ok(transient)
}
