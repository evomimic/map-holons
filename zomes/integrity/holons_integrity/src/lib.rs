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

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    #[allow(unreachable_patterns)]
    match op.flattened::<EntryTypes, LinkTypes>()? {
        FlatOp::StoreEntry(store_entry) => match store_entry {
            OpEntry::CreateEntry { app_entry, action } => match app_entry {
                EntryTypes::HolonNode(holon_node) => {
                    let persistence_create = PersistenceCreate::new(
                        persistence_agent_id_from_agent_pub_key(action.author),
                        PersistenceTimestamp(action.timestamp.0),
                        action.action_seq,
                        local_id_from_action_hash(action.prev_action),
                    );
                    let holon_node_model =
                        HolonNodeModel::new(holon_node.original_id, holon_node.property_map);
                    validate_create_holon_node(
                        PersistenceAction::Create(persistence_create),
                        holon_node_model,
                    )
                }
            },
            OpEntry::UpdateEntry { app_entry, action, .. } => match app_entry {
                EntryTypes::HolonNode(holon_node) => {
                    let holon_node_model =
                        HolonNodeModel::new(holon_node.original_id, holon_node.property_map);
                    let persistence_update = PersistenceUpdate::new(
                        persistence_agent_id_from_agent_pub_key(action.author),
                        PersistenceTimestamp(action.timestamp.0),
                        action.action_seq,
                        local_id_from_action_hash(action.prev_action),
                    );
                    validate_create_holon_node(
                        PersistenceAction::Update(persistence_update),
                        holon_node_model,
                    )
                }
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::RegisterUpdate(update_entry) => match update_entry {
            OpUpdate::Entry { app_entry, action } => match app_entry {
                EntryTypes::HolonNode(holon_node) => {
                    let holon_node_model =
                        HolonNodeModel::new(holon_node.original_id, holon_node.property_map);
                    let persistence_update = PersistenceUpdate::new(
                        persistence_agent_id_from_agent_pub_key(action.author),
                        PersistenceTimestamp(action.timestamp.0),
                        action.action_seq,
                        local_id_from_action_hash(action.prev_action),
                    );
                    validate_update_holon_node(persistence_update, holon_node_model)
                }
                _ => Ok(ValidateCallbackResult::Invalid(
                    "Original and updated entry types must be the same".to_string(),
                )),
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::RegisterDelete(delete_entry) => match delete_entry {
            OpDelete { action } => {
                let persistence_delete = PersistenceDelete::new(
                    persistence_agent_id_from_agent_pub_key(action.author),
                    PersistenceTimestamp(action.timestamp.0),
                    action.action_seq,
                    local_id_from_action_hash(action.prev_action),
                );
                validate_delete_holon_node(persistence_delete)
            }
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
            OpRecord::CreateEntry { app_entry, action } => match app_entry {
                EntryTypes::HolonNode(holon_node) => {
                    let holon_node_model =
                        HolonNodeModel::new(holon_node.original_id, holon_node.property_map);
                    let persistence_create = PersistenceCreate::new(
                        persistence_agent_id_from_agent_pub_key(action.author),
                        PersistenceTimestamp(action.timestamp.0),
                        action.action_seq,
                        local_id_from_action_hash(action.prev_action),
                    );
                    validate_create_holon_node(
                        PersistenceAction::Create(persistence_create),
                        holon_node_model,
                    )
                }
            },
            OpRecord::UpdateEntry { original_action_hash, app_entry, action, .. } => {
                let original_record = must_get_valid_record(original_action_hash)?;
                let original_action = original_record.action().clone();
                let _original_action = match original_action {
                    Action::Create(create) => EntryCreationAction::Create(create),
                    Action::Update(update) => EntryCreationAction::Update(update),
                    _ => {
                        return Ok(ValidateCallbackResult::Invalid(
                            "Original action for an update must be a Create or Update action"
                                .to_string(),
                        ));
                    }
                };
                match app_entry {
                    EntryTypes::HolonNode(holon_node) => {
                        let holon_node_model =
                            HolonNodeModel::new(holon_node.original_id, holon_node.property_map);
                        let persistence_update = PersistenceUpdate::new(
                            persistence_agent_id_from_agent_pub_key(action.author.clone()),
                            PersistenceTimestamp(action.timestamp.0),
                            action.action_seq,
                            local_id_from_action_hash(action.prev_action.clone()),
                        );
                        let result = validate_create_holon_node(
                            PersistenceAction::Update(persistence_update),
                            holon_node_model.clone(),
                        )?;
                        if let ValidateCallbackResult::Valid = result {
                            let original_holon_node: Option<HolonNode> = original_record
                                .entry()
                                .to_app_option()
                                .map_err(|e| wasm_error!(e))?;
                            let _original_holon_node = match original_holon_node {
                                Some(holon_node) => holon_node,
                                None => {
                                    return Ok(
                                            ValidateCallbackResult::Invalid(
                                                "The updated entry type must be the same as the original entry type"
                                                    .to_string(),
                                            ),
                                        );
                                }
                            };
                            let persistence_update = PersistenceUpdate::new(
                                persistence_agent_id_from_agent_pub_key(action.author),
                                PersistenceTimestamp(action.timestamp.0),
                                action.action_seq,
                                local_id_from_action_hash(action.prev_action),
                            );
                            validate_update_holon_node(persistence_update, holon_node_model)
                        } else {
                            Ok(result)
                        }
                    }
                }
            }
            OpRecord::DeleteEntry { original_action_hash, action, .. } => {
                let original_record = must_get_valid_record(original_action_hash)?;
                let original_action = original_record.action().clone();
                let original_action = match original_action {
                    Action::Create(create) => EntryCreationAction::Create(create),
                    Action::Update(update) => EntryCreationAction::Update(update),
                    _ => {
                        return Ok(ValidateCallbackResult::Invalid(
                            "Original action for a delete must be a Create or Update action"
                                .to_string(),
                        ));
                    }
                };
                let app_entry_type = match original_action.entry_type() {
                    EntryType::App(app_entry_type) => app_entry_type,
                    _ => {
                        return Ok(ValidateCallbackResult::Valid);
                    }
                };
                let entry = match original_record.entry().as_option() {
                    Some(entry) => entry,
                    None => {
                        return if original_action.entry_type().visibility().is_public() {
                            Ok(
                                    ValidateCallbackResult::Invalid(
                                        "Original record for a delete of a public entry must contain an entry"
                                            .to_string(),
                                    ),
                                )
                        } else {
                            Ok(ValidateCallbackResult::Valid)
                        };
                    }
                };
                let original_app_entry = match EntryTypes::deserialize_from_type(
                    app_entry_type.zome_index.clone(),
                    app_entry_type.entry_index.clone(),
                    &entry,
                )? {
                    Some(app_entry) => app_entry,
                    None => {
                        return Ok(
                                ValidateCallbackResult::Invalid(
                                    "Original app entry must be one of the defined entry types for this zome"
                                        .to_string(),
                                ),
                            );
                    }
                };
                match original_app_entry {
                    EntryTypes::HolonNode(_original_holon_node) => {
                        let persistence_delete = PersistenceDelete::new(
                            persistence_agent_id_from_agent_pub_key(action.author),
                            PersistenceTimestamp(action.timestamp.0),
                            action.action_seq,
                            local_id_from_action_hash(action.prev_action),
                        );
                        validate_delete_holon_node(persistence_delete)
                    }
                }
            }
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
