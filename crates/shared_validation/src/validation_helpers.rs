/// Foundational routines for property and relationship checks
/// applicable to both zomes
use crate::ValidationError;
use shared_types_holon::HolonNode;

// Example
pub fn validate_holon_property(_holon: &HolonNode) -> Result<(), Vec<ValidationError>> {
    Ok(())
}
