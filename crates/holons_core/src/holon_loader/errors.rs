use base_types::{BaseValue, MapInteger, MapString};
use core_types::HolonError;

use crate::{core_shared_objects::holon::TransientHolon, HolonReference, HolonsContextBehavior, TransientReference};

use super::names as N;

/// Convert a HolonError into a stable, snake_case error_type string for the HolonError holon.
pub fn error_type_code(err: &HolonError) -> &'static str {
    use HolonError::*;
    match err {
        // New, loader-specific
        HolonError::LoaderInputMalformed(_)     => "loader_input_malformed",
        HolonError::ReferenceResolutionFailed(_) => "reference_resolution_failed",
        HolonError::InverseMappingNotFound(_)   => "inverse_mapping_not_found",
        HolonError::DescriptorResolutionFailed(_) => "descriptor_resolution_failed",

        // Common/existing (only list those you expect from the loader path)
        HolonError::DuplicateError(_, _)        => "duplicate",
        HolonError::InvalidRelationship(_, _)   => "invalid_relationship",
        HolonError::ValidationError(_)          => "validation_error",
        HolonError::CommitFailure(_)            => "commit_failure",
        HolonError::UnexpectedValueType(_, _)   => "unexpected_value_type",
        HolonError::InvalidType(_)              => "invalid_type",
        HolonError::HolonNotFound(_)            => "holon_not_found",
        // HolonError::BadRequest(_)=> "bad_request", // if you add one later
        _                                        => "server_error",
    }
}

/// Build a transient HolonError holon with {error_type, error_message}.
/// Returns a TransientReference so the controller can `add_related_holons(..., HAS_LOAD_ERROR, ...)`.
pub fn make_error_holon(
    context: &dyn HolonsContextBehavior,
    err: &HolonError,
) -> Result<TransientReference, HolonError> {
    let mut e = TransientHolon::new();
    // Set the descriptor to HolonError.Type
    e.with_descriptor(context, holon_error_type_descriptor_ref(context)?)?;
    // Set properties
    e.update_property_map(context, vec![
        (N::prop(N::PROP_ERROR_TYPE),    MapString(error_type_code(err).to_string()).into()),
        (N::prop(N::PROP_ERROR_MESSAGE), MapString(err.to_string()).into()),
    ])?;
    e.as_transient_reference(context)
}

/// Helper to fetch (or otherwise derive) the descriptor ref for HolonError.Type.
/// You may already have a constant or lookup utility—wire it here.
fn holon_error_type_descriptor_ref(
    context: &dyn HolonsContextBehavior
) -> Result<HolonReference, HolonError> {
    // e.g., resolve by key "HolonError.Type" in the local space,
    // or use a known baked-in reference if you maintain those.
    // return find_descriptor_by_key(context, "HolonError.Type");
    unimplemented!()
}
