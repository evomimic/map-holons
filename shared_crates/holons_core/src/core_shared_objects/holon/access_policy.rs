use core_types::HolonError;

use super::state::AccessType;

/// Standard saved-holon capabilities, shared by saved objects and references.
///
/// This policy requires no resolved holon state. Reads remain available for
/// validation and inspection regardless of deletion or validation status.
/// Callers that require additional state-dependent eligibility must assess it
/// explicitly; ordinary reads must not hydrate a holon merely to check access.
pub(crate) fn check_saved_holon_access(access_type: AccessType) -> Result<(), HolonError> {
    match access_type {
        AccessType::Read | AccessType::Clone => Ok(()),
        AccessType::Write | AccessType::Commit | AccessType::Abandon => {
            Err(HolonError::NotAccessible(access_type.to_string(), "Saved".to_string()))
        }
    }
}
