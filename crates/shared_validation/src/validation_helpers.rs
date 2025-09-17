use core_types::ValidationError;
use holons_core::core_shared_objects::{Holon, TransientHolon};
use integrity_core_types::{HolonNodeModel, LocalId, PersistenceLinkTag, PropertyMap};

/// Foundational routines for property and relationship checks
/// applicable to both zomes

// ==== Entry CUD ====

pub fn validate_create_holon(holon_node_model: HolonNodeModel) -> Result<(), ValidationError> {
    // Deferring logic until Descriptors

    Ok(())
}

pub fn validate_update_holon(holon_node_model: HolonNodeModel) -> Result<(), ValidationError> {
    Ok(())
}

pub fn validate_delete_holon() -> Result<(), ValidationError> {
    Ok(())
}

// ==== Smartlink ====

pub fn validate_create_smartlink_helper(
    base_address: LocalId,
    target_address: LocalId,
    tag: PersistenceLinkTag,
) -> Result<(), ValidationError> {
    Ok(())
}

pub fn validate_delete_smartlink_helper(
    base: LocalId,
    target: LocalId,
) -> Result<(), ValidationError> {
    Ok(())
}
