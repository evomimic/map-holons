/// Foundational routines for property and relationship checks
/// applicable to both zomes
use crate::ValidationError;
use integrity_core_types::HolonNodeModel;

// Example
pub fn validate_holon_property(_holon: &HolonNodeModel) -> Result<(), Vec<ValidationError>> {
    Ok(())
}
