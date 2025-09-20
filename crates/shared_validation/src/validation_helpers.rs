use core_types::ValidationError;
use integrity_core_types::{HolonNodeModel, LocalId, PersistenceLinkTag};

/// Foundational routines for property and relationship checks
/// applicable to both zomes

// ==== Entry CUD ====

pub fn validate_create_holon(_holon_node_model: HolonNodeModel) -> Result<(), ValidationError> {
    // Deferring logic until Descriptors

    Ok(())
}

pub fn validate_update_holon(_holon_node_model: HolonNodeModel) -> Result<(), ValidationError> {
    Ok(())
}

pub fn validate_delete_holon() -> Result<(), ValidationError> {
    Ok(())
}

// ==== Smartlink ====

pub fn validate_create_smartlink_helper(
    _base_address: LocalId,
    _target_address: LocalId,
    _tag: PersistenceLinkTag,
) -> Result<(), ValidationError> {
    Ok(())
}

pub fn validate_delete_smartlink_helper(
    _base: LocalId,
    _target: LocalId,
) -> Result<(), ValidationError> {
    Ok(())
}
