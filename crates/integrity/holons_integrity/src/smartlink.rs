use hdi::prelude::*;
//use shared_types_holon::holon_node::{HolonNode};

pub const EXTERNAL_REFERENCE_TYPE: [u8; 3] = [226, 147, 141]; // Unicode 'Ⓧ' // hex bytes: [0xE2] [0x93] [0x8D]
pub const LOCAL_REFERENCE_TYPE: [u8; 3] = [226, 147, 129]; // Unicode 'Ⓛ' // hex bytes: [0xE2] [0x93] [0x81]
pub const NUL_BYTES: u8 = b'\0'; // NUL Bytes
pub const PROLOG_SEPARATOR: [u8; 3] = [226, 138, 163]; // Unicode '⊣' // hex bytes: [0xE2][0x8A][0xA3]
pub const PROPERTY_NAME_SEPARATOR: [u8; 3] = [226, 147, 131]; // Unicode 'Ⓝ' // hex bytes: [0xE2][0x93][0x83]
pub const PROPERTY_VALUE_SEPARATOR: [u8; 3] = [226, 147, 11]; // Unicode 'Ⓥ' // hex bytes: [0xE2][0x93][0x8B]
pub const PROXY_ID_SEPARATOR: &str = "\u{0}"; // Unicode NUL character // hex bytes: [0x00]
pub const RELATIONSHIP_NAME_SEPARATOR: &str = "\u{0}"; // Unicode NUL character // hex bytes: [0x00]
pub const SMARTLINK_HEADER_BYTES: [u8; 3] = [226, 130, 183]; // Unicode '₷' // hex bytes: [0xE2][0x82][0xB7]
pub const UNICODE_NUL_STR: &str = "\u{0}"; // Unicode NUL character // hex bytes: [0x00]

pub fn validate_create_smartlink(
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
pub fn validate_delete_smartlink(
    _action: DeleteLink,
    _original_action: CreateLink,
    _base: AnyLinkableHash,
    _target: AnyLinkableHash,
    _tag: LinkTag,
) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
