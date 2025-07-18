use hdi::prelude::*;
use holons_guest_integrity::HolonNode;

pub fn validate_create_holon_node(
    _action: EntryCreationAction,
    _holon_node: HolonNode,
) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_update_holon_node(
    _action: Update,
    _holon_node: HolonNode,
) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_delete_holon_node(_action: Delete) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_create_link_holon_node_updates(
    _action: CreateLink,
    base_address: AnyLinkableHash,
    target_address: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    let action_hash = base_address.into_action_hash().ok_or(wasm_error!(WasmErrorInner::Guest(
        String::from("No action hash associated with link")
    )))?;
    let record = must_get_valid_record(action_hash)?;
    let _holon_node: crate::HolonNode =
        record.entry().to_app_option().map_err(|e| wasm_error!(e))?.ok_or(wasm_error!(
            WasmErrorInner::Guest(String::from("Linked action must reference an entry"))
        ))?;
    let action_hash = target_address.into_action_hash().ok_or(wasm_error!(
        WasmErrorInner::Guest(String::from("No action hash associated with link"))
    ))?;
    let record = must_get_valid_record(action_hash)?;
    let _holon_node: crate::HolonNode =
        record.entry().to_app_option().map_err(|e| wasm_error!(e))?.ok_or(wasm_error!(
            WasmErrorInner::Guest(String::from("Linked action must reference an entry"))
        ))?;
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_delete_link_holon_node_updates(
    _action: DeleteLink,
    _original_action: CreateLink,
    _base: AnyLinkableHash,
    _target: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Invalid(String::from("HolonNodeUpdates links cannot be deleted")))
}

pub fn validate_create_link_all_holon_nodes(
    _action: CreateLink,
    _base_address: AnyLinkableHash,
    target_address: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    let action_hash = target_address.into_action_hash().ok_or(wasm_error!(
        WasmErrorInner::Guest(String::from("No action hash associated with link"))
    ))?;
    let record = must_get_valid_record(action_hash)?;
    let _holon_node: crate::HolonNode =
        record.entry().to_app_option().map_err(|e| wasm_error!(e))?.ok_or(wasm_error!(
            WasmErrorInner::Guest(String::from("Linked action must reference an entry"))
        ))?;
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_delete_link_all_holon_nodes(
    _action: DeleteLink,
    _original_action: CreateLink,
    _base: AnyLinkableHash,
    _target: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Invalid(String::from("AllHolonNodes links cannot be deleted")))
}

pub fn validate_create_link_local_holon_space(
    _action: CreateLink,
    _base_address: AnyLinkableHash,
    target_address: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    // Check the entry type for the given action hash
    let action_hash = target_address.into_action_hash().ok_or(wasm_error!(
        WasmErrorInner::Guest("No action hash associated with link".to_string())
    ))?;
    let record = must_get_valid_record(action_hash)?;
    let _holon_node: crate::HolonNode =
        record.entry().to_app_option().map_err(|e| wasm_error!(e))?.ok_or(wasm_error!(
            WasmErrorInner::Guest("Linked action must reference an entry".to_string())
        ))?;
    // TODO: add the appropriate validation rules
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_delete_link_local_holon_space(
    _action: DeleteLink,
    _original_action: CreateLink,
    _base: AnyLinkableHash,
    _target: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    // TODO: add the appropriate validation rules
    Ok(ValidateCallbackResult::Valid)
}
