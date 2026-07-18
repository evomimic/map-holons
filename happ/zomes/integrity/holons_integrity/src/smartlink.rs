use hdi::prelude::*;
use integrity_core_types::{
    LocalId, PersistenceCreateLink, PersistenceDeleteLink, PersistenceLinkTag,
};
use shared_validation::*;

pub fn validate_create_smartlink(
    _action: PersistenceCreateLink,
    base_address: LocalId,
    target_address: LocalId,
    tag: PersistenceLinkTag,
) -> ExternResult<ValidateCallbackResult> {
    Ok(match validate_create_smartlink_helper(base_address, target_address, tag) {
        Ok(()) => ValidateCallbackResult::Valid,
        Err(error) => ValidateCallbackResult::Invalid(error.to_string()),
    })
}
pub fn validate_delete_smartlink(
    _action: PersistenceDeleteLink,
    _original_action: PersistenceCreateLink,
    base: LocalId,
    target: LocalId,
    _tag: PersistenceLinkTag,
) -> ExternResult<ValidateCallbackResult> {
    validate_delete_smartlink_helper(base, target)
        .map_err(|e| wasm_error!(WasmErrorInner::Guest(e.to_string())))?;

    Ok(ValidateCallbackResult::Valid)
}
