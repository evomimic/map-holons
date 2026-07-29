//! # Integrity Zome Validation Logic
//!
//! This file implements the integrity zome interface functions expected by the Holochain Conductor.
//!
//! Specifically, it:
//! - Initiates the per-event validation process by responding to Holochain's `validate` extern call.
//! - Handles all inbound validation requests and dispatches them to holochain-independent validation functions,
//!   primarily located in the `shared_validations` crate.
//! - Translates all Holochain-specific types (e.g., `Op`, `Action`, `EntryTypes`, `LinkTypes`) and validation events
//!   into persistence-layer abstractions used throughout the Memetic Activation Platform.
//!
//! The purpose of this layer is to isolate Holochain's runtime environment from the domain validation logic,
//! providing a clean separation of concerns and enabling testing and reuse of validation logic outside of the
//! Holochain execution context.

use hdi::prelude::*;

use holons_guest_integrity::*;
use integrity_core_types::*;

pub mod holon_node;
pub mod smartlink;

pub use holon_node::*;
pub use smartlink::*;

#[cfg(test)]
mod tests;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    HolonNode(HolonNode),
}

#[derive(Serialize, Deserialize)]
#[hdk_link_types]
pub enum LinkTypes {
    AllHolonNodes,
    HolonNodeUpdates,
    LocalHolonSpace,
    SmartLink,
}

#[hdk_extern]
pub fn genesis_self_check(_data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

pub fn validate_agent_joining(
    _agent_pub_key: AgentPubKey,
    _membrane_proof: &Option<MembraneProof>,
) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

/// Maps a completed lifecycle verdict onto the Integrity callback contract.
///
/// Host and dependency-resolution failures are propagated before this helper is
/// called. Only deterministic PVL violations become consensus-visible
/// `Invalid` results.
fn lifecycle_callback_result(result: Result<(), PvlViolation>) -> ValidateCallbackResult {
    match result {
        Ok(()) => ValidateCallbackResult::Valid,
        Err(violation) => ValidateCallbackResult::Invalid(violation.to_string()),
    }
}

/// Validates one flattened `HolonNode` update through the shared adapter path.
///
/// All three update op arms delegate here so target extraction, scoped
/// app-entry classification, and violation mapping cannot drift independently.
fn validate_holon_node_update_arm(
    original_action_hash: &ActionHash,
    update: &Update,
) -> ExternResult<ValidateCallbackResult> {
    let EntryType::App(new_entry_def) = &update.entry_type else {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "flattened app-entry update did not carry an App entry type".into()
        )));
    };

    Ok(lifecycle_callback_result(validate_holon_node_update_target(
        original_action_hash,
        new_entry_def,
    )?))
}

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match prepare_holon_node_envelope(&op)? {
        HolonNodeEnvelope::Invalid(violation) => {
            return Ok(ValidateCallbackResult::Invalid(violation.to_string()));
        }
        HolonNodeEnvelope::Valid(_) | HolonNodeEnvelope::NotApplicable => {}
    }

    #[allow(unreachable_patterns)]
    match op.flattened::<EntryTypes, LinkTypes>()? {
        // HolonNode envelope validation already succeeded in the raw-op guard above.
        FlatOp::StoreEntry(store_entry) => match store_entry {
            OpEntry::CreateEntry { app_entry, .. } => match app_entry {
                EntryTypes::HolonNode(_) => Ok(ValidateCallbackResult::Valid),
            },
            OpEntry::UpdateEntry { app_entry, original_action_hash, action, .. } => match app_entry
            {
                EntryTypes::HolonNode(_) => {
                    validate_holon_node_update_arm(&original_action_hash, &action)
                }
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::RegisterUpdate(update_entry) => match update_entry {
            // Storage SL2 will activate this path when version-producing writes become native,
            // root-addressed updates targeting the lineage-root Create. That transition also
            // removes original_id from the entry shape and requires parity-fixture review.
            OpUpdate::Entry { app_entry, action } => match app_entry {
                EntryTypes::HolonNode(_) => {
                    validate_holon_node_update_arm(&action.original_action_address, &action)
                }
                _ => Ok(ValidateCallbackResult::Invalid(
                    "Original and updated entry types must be the same".to_string(),
                )),
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::RegisterDelete(delete_entry) => match delete_entry {
            OpDelete { action } => Ok(lifecycle_callback_result(
                validate_holon_node_delete_target(&action.deletes_address)?,
            )),
        },
        FlatOp::RegisterCreateLink { link_type, base_address, target_address, tag, action } => {
            match link_type {
                LinkTypes::HolonNodeUpdates => validate_create_link_holon_node_updates(
                    action,
                    base_address,
                    target_address,
                    tag,
                ),
                LinkTypes::SmartLink => {
                    let persistence_create_link = PersistenceCreateLink::new(
                        persistence_agent_id_from_agent_pub_key(action.author),
                        PersistenceTimestamp(action.timestamp.0),
                        action.action_seq,
                        local_id_from_action_hash(action.prev_action),
                        local_id_from_action_hash(action.base_address.into_action_hash().ok_or(
                            wasm_error!(WasmErrorInner::Guest(String::from(
                                "No action hash associated with link"
                            ))),
                        )?),
                        local_id_from_action_hash(action.target_address.into_action_hash().ok_or(
                            wasm_error!(WasmErrorInner::Guest(String::from(
                                "No action hash associated with link"
                            ))),
                        )?),
                        PersistenceLinkType::SmartLink,
                        PersistenceLinkTag(action.tag.into_inner()),
                    );

                    let base_local_id =
                        local_id_from_action_hash(base_address.into_action_hash().ok_or(
                            wasm_error!(WasmErrorInner::Guest(String::from(
                                "No action hash associated with link"
                            ))),
                        )?);
                    let target_local_id =
                        local_id_from_action_hash(target_address.into_action_hash().ok_or(
                            wasm_error!(WasmErrorInner::Guest(String::from(
                                "No action hash associated with link"
                            ))),
                        )?);

                    validate_create_smartlink(
                        persistence_create_link,
                        base_local_id,
                        target_local_id,
                        PersistenceLinkTag(tag.into_inner()),
                    )
                }
                LinkTypes::AllHolonNodes => {
                    validate_create_link_all_holon_nodes(action, base_address, target_address, tag)
                }
                LinkTypes::LocalHolonSpace => validate_create_link_local_holon_space(
                    action,
                    base_address,
                    target_address,
                    tag,
                ),
            }
        }
        FlatOp::RegisterDeleteLink {
            link_type,
            base_address,
            target_address,
            tag,
            original_action,
            action,
        } => match link_type {
            LinkTypes::HolonNodeUpdates => validate_delete_link_holon_node_updates(
                action,
                original_action,
                base_address,
                target_address,
                tag,
            ),
            LinkTypes::SmartLink => {
                let persistence_delete_link = PersistenceDeleteLink::new(
                    persistence_agent_id_from_agent_pub_key(action.author),
                    PersistenceTimestamp(action.timestamp.0),
                    action.action_seq,
                    local_id_from_action_hash(action.prev_action),
                    local_id_from_action_hash(action.base_address.into_action_hash().ok_or(
                        wasm_error!(WasmErrorInner::Guest(String::from(
                            "No action hash associated with link"
                        ))),
                    )?),
                );

                let base_local_id = local_id_from_action_hash(
                    base_address.into_action_hash().ok_or(wasm_error!(WasmErrorInner::Guest(
                        String::from("No action hash associated with link")
                    )))?,
                );
                let target_local_id = local_id_from_action_hash(
                    target_address.into_action_hash().ok_or(wasm_error!(WasmErrorInner::Guest(
                        String::from("No action hash associated with link")
                    )))?,
                );

                let persistence_create_link = PersistenceCreateLink::new(
                    persistence_agent_id_from_agent_pub_key(original_action.author),
                    PersistenceTimestamp(original_action.timestamp.0),
                    original_action.action_seq,
                    local_id_from_action_hash(original_action.prev_action),
                    local_id_from_action_hash(
                        original_action.base_address.into_action_hash().ok_or(wasm_error!(
                            WasmErrorInner::Guest(String::from(
                                "No action hash associated with link"
                            ))
                        ))?,
                    ),
                    local_id_from_action_hash(
                        original_action.target_address.into_action_hash().ok_or(wasm_error!(
                            WasmErrorInner::Guest(String::from(
                                "No action hash associated with link"
                            ))
                        ))?,
                    ),
                    PersistenceLinkType::SmartLink,
                    PersistenceLinkTag(original_action.tag.into_inner()),
                );

                validate_delete_smartlink(
                    persistence_delete_link,
                    persistence_create_link,
                    base_local_id,
                    target_local_id,
                    PersistenceLinkTag(tag.into_inner()),
                )
            }
            LinkTypes::AllHolonNodes => validate_delete_link_all_holon_nodes(
                action,
                original_action,
                base_address,
                target_address,
                tag,
            ),
            LinkTypes::LocalHolonSpace => validate_delete_link_local_holon_space(
                action,
                original_action,
                base_address,
                target_address,
                tag,
            ),
        },
        FlatOp::StoreRecord(store_record) => match store_record {
            // HolonNode envelope validation already succeeded in the raw-op guard above.
            OpRecord::CreateEntry { app_entry, .. } => match app_entry {
                EntryTypes::HolonNode(_) => Ok(ValidateCallbackResult::Valid),
            },
            // MAP writes currently emit Creates. Storage SL2 will exercise this path with native
            // updates targeting the lineage-root Create; envelope preparation has already run.
            OpRecord::UpdateEntry { original_action_hash, app_entry, action, .. } => {
                match app_entry {
                    EntryTypes::HolonNode(_) => {
                        validate_holon_node_update_arm(&original_action_hash, &action)
                    }
                }
            }
            OpRecord::DeleteEntry { action, .. } => Ok(lifecycle_callback_result(
                validate_holon_node_delete_target(&action.deletes_address)?,
            )),
            OpRecord::CreateLink { base_address, target_address, tag, link_type, action } => {
                match link_type {
                    LinkTypes::HolonNodeUpdates => validate_create_link_holon_node_updates(
                        action,
                        base_address,
                        target_address,
                        tag,
                    ),
                    LinkTypes::SmartLink => {
                        let persistence_create_link = PersistenceCreateLink::new(
                            persistence_agent_id_from_agent_pub_key(action.author),
                            PersistenceTimestamp(action.timestamp.0),
                            action.action_seq,
                            local_id_from_action_hash(action.prev_action),
                            local_id_from_action_hash(
                                action.base_address.into_action_hash().ok_or(wasm_error!(
                                    WasmErrorInner::Guest(String::from(
                                        "No action hash associated with link"
                                    ))
                                ))?,
                            ),
                            local_id_from_action_hash(
                                action.target_address.into_action_hash().ok_or(wasm_error!(
                                    WasmErrorInner::Guest(String::from(
                                        "No action hash associated with link"
                                    ))
                                ))?,
                            ),
                            PersistenceLinkType::SmartLink,
                            PersistenceLinkTag(action.tag.into_inner()),
                        );

                        let base_local_id =
                            local_id_from_action_hash(base_address.into_action_hash().ok_or(
                                wasm_error!(WasmErrorInner::Guest(String::from(
                                    "No action hash associated with link"
                                ))),
                            )?);
                        let target_local_id =
                            local_id_from_action_hash(target_address.into_action_hash().ok_or(
                                wasm_error!(WasmErrorInner::Guest(String::from(
                                    "No action hash associated with link"
                                ))),
                            )?);

                        validate_create_smartlink(
                            persistence_create_link,
                            base_local_id,
                            target_local_id,
                            PersistenceLinkTag(tag.into_inner()),
                        )
                    }
                    LinkTypes::AllHolonNodes => validate_create_link_all_holon_nodes(
                        action,
                        base_address,
                        target_address,
                        tag,
                    ),
                    LinkTypes::LocalHolonSpace => validate_create_link_local_holon_space(
                        action,
                        base_address,
                        target_address,
                        tag,
                    ),
                }
            }
            OpRecord::DeleteLink { original_action_hash, base_address, action } => {
                let record = must_get_valid_record(original_action_hash)?;
                let create_link = match record.action() {
                    Action::CreateLink(create_link) => create_link.clone(),
                    _ => {
                        return Ok(ValidateCallbackResult::Invalid(
                            "The action that a DeleteLink deletes must be a CreateLink".to_string(),
                        ));
                    }
                };
                let link_type = match LinkTypes::from_type(
                    create_link.zome_index.clone(),
                    create_link.link_type.clone(),
                )? {
                    Some(lt) => lt,
                    None => {
                        return Ok(ValidateCallbackResult::Valid);
                    }
                };
                match link_type {
                    LinkTypes::HolonNodeUpdates => validate_delete_link_holon_node_updates(
                        action,
                        create_link.clone(),
                        base_address,
                        create_link.target_address,
                        create_link.tag,
                    ),
                    LinkTypes::SmartLink => {
                        let persistence_delete_link = PersistenceDeleteLink::new(
                            persistence_agent_id_from_agent_pub_key(action.author),
                            PersistenceTimestamp(action.timestamp.0),
                            action.action_seq,
                            local_id_from_action_hash(action.prev_action),
                            local_id_from_action_hash(
                                action.base_address.into_action_hash().ok_or(wasm_error!(
                                    WasmErrorInner::Guest(String::from(
                                        "No action hash associated with link"
                                    ))
                                ))?,
                            ),
                        );

                        let tag = PersistenceLinkTag(create_link.tag.into_inner());

                        let base_local_id =
                            local_id_from_action_hash(base_address.into_action_hash().ok_or(
                                wasm_error!(WasmErrorInner::Guest(String::from(
                                    "No action hash associated with link"
                                ))),
                            )?);
                        let target_local_id = local_id_from_action_hash(
                            create_link.target_address.into_action_hash().ok_or(wasm_error!(
                                WasmErrorInner::Guest(String::from(
                                    "No action hash associated with link"
                                ))
                            ))?,
                        );

                        let persistence_create_link = PersistenceCreateLink::new(
                            persistence_agent_id_from_agent_pub_key(create_link.author),
                            PersistenceTimestamp(create_link.timestamp.0),
                            create_link.action_seq,
                            local_id_from_action_hash(create_link.prev_action),
                            base_local_id.clone(),
                            target_local_id.clone(),
                            PersistenceLinkType::SmartLink,
                            tag.clone(),
                        );

                        validate_delete_smartlink(
                            persistence_delete_link,
                            persistence_create_link,
                            base_local_id,
                            target_local_id,
                            tag,
                        )
                    }
                    LinkTypes::AllHolonNodes => validate_delete_link_all_holon_nodes(
                        action,
                        create_link.clone(),
                        base_address,
                        create_link.target_address,
                        create_link.tag,
                    ),
                    LinkTypes::LocalHolonSpace => validate_delete_link_local_holon_space(
                        action,
                        create_link.clone(),
                        base_address,
                        create_link.target_address,
                        create_link.tag,
                    ),
                }
            }
            OpRecord::CreatePrivateEntry { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::UpdatePrivateEntry { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::CreateCapClaim { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::CreateCapGrant { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::UpdateCapClaim { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::UpdateCapGrant { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::Dna { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::OpenChain { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::CloseChain { .. } => Ok(ValidateCallbackResult::Valid),
            OpRecord::InitZomesComplete { .. } => Ok(ValidateCallbackResult::Valid),
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::RegisterAgentActivity(agent_activity) => match agent_activity {
            OpActivity::CreateAgent { agent, action } => {
                let previous_action = must_get_action(action.prev_action)?;
                match previous_action.action() {
                        Action::AgentValidationPkg(
                            AgentValidationPkg { membrane_proof, .. },
                        ) => validate_agent_joining(agent, membrane_proof),
                        _ => {
                            Ok(
                                ValidateCallbackResult::Invalid(
                                    "The previous action for a `CreateAgent` action must be an `AgentValidationPkg`"
                                        .to_string(),
                                ),
                            )
                        }
                    }
            }
            _ => Ok(ValidateCallbackResult::Valid),
        },
    }
}
