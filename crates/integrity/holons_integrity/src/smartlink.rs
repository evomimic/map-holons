use hdi::prelude::*;
//use shared_types_holon::holon_node::{HolonNode};
pub fn validate_create_smartlink(
    _action: CreateLink,
    base_address: AnyLinkableHash,
    target_address: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    let action_hash = base_address.into_action_hash().ok_or(wasm_error!(WasmErrorInner::Guest(String::from("No action hash associated with link"))),)?;
    let record = must_get_valid_record(action_hash)?;
    let _holon_node: crate::HolonNode = record
        .entry()
        .to_app_option()
        .map_err(|e| wasm_error!(e))?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Linked action must reference an entry"))
            ),
        )?;
    let action_hash = target_address.into_action_hash().ok_or(wasm_error!(WasmErrorInner::Guest(String::from("No action hash associated with link"))),)?;
    let record = must_get_valid_record(action_hash)?;
    let _holon_node: crate::HolonNode = record
        .entry()
        .to_app_option()
        .map_err(|e| wasm_error!(e))?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Linked action must reference an entry"))
            ),
        )?;
    Ok(ValidateCallbackResult::Valid)
}
pub fn validate_delete_smartlink(
    _action: DeleteLink,
    _original_action: CreateLink,
    _base: AnyLinkableHash,
    _target: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
