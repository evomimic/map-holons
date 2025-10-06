use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use hdk::prelude::*;
use holons_guest_integrity::type_conversions::*;
use holons_guest_integrity::HolonNode;

// use crate::{
//     create_holon_node, save_smartlink, update_holon_node, SmartLink, UpdateHolonNodeInput,
// };
use crate::guest_shared_objects::{save_smartlink, SmartLink};
use crate::persistence_layer::{create_holon_node, update_holon_node, UpdateHolonNodeInput};

use holons_core::{
    core_shared_objects::{
        holon::state::{AccessType, StagedState},
        CommitRequestStatus, CommitResponse, Holon, ReadableHolonState, HolonCollection, StagedHolon,
    },
    reference_layer::{HolonsContextBehavior, ReadableHolon},
};
// use holons_core::utils::as_json;
use base_types::{BaseValue, MapInteger, MapString};
use core_types::HolonError;
use integrity_core_types::{LocalId, PropertyMap, PropertyName, RelationshipName};

/// `commit`
///
/// This function attempts to persist the state of all staged_holons AND their relationships.
/// It is not completely Atomic - since successfully committed holons will get saved to the Holochain persistance store,
/// however, the CommitResponse is only considered 'Complete' if ALL the attempts are successful , as well as,
/// the space_manager's staged_holons are only cleared IF this is the case. Otherwise, the failed holons StagedState will remain unchanged
/// and their data objects will contain the associated errors that were returned.
///
///
/// A further description of this process is detailed below:
///
/// The commit is performed in two passes: (1) staged_holons, (2) their relationships.
///
/// In the first pass,
/// * if a staged_holon commit succeeds,
///     * get the LocalId from the action_address in the returned Record
///     * StagedState variant is set to 'Committed' containing the above 'saved_id'
///     * add the holon to the saved_holons vector in the CommitResponse
/// * if a staged_holon commit fails,
///     * leave holon's state unchanged
///     * push the associated error into the holon's errors vector
///     * do NOT add the holon to the saved_holons vector in the CommitResponse
///
/// If ANY staged_holon commit fails:
/// * The 2nd pass (to commit the staged_holon's relationships) is SKIPPED
/// * the overall return status in the CommitResponse is set to `Incomplete`
/// * the function returns.
///
/// Otherwise, the 2nd pass is performed.
/// * If ANY attempt to add a relationship generates an Error, the error is pushed into the
/// source holon's `errors` vector and processing continues.
///
/// If relationship commits succeed for ALL staged_holons,
///     * The space_manager's staged_holons are cleared
///     * The Commit Response returns a `Complete` status
///
/// NOTE: The CommitResponse returns clones of any successfully
/// committed holons, even if the response status is `Incomplete`.
///
pub fn commit(
    context: &dyn HolonsContextBehavior,
    staged_holons: &Vec<Arc<RwLock<Holon>>>,
) -> Result<CommitResponse, HolonError> {
    info!("Entering commit...");

    // Initialize the request_status to Complete, assuming all commits will succeed
    // If any commit errors are encountered, reset request_status to `Incomplete`
    let mut response = CommitResponse {
        status: CommitRequestStatus::Complete,
        commits_attempted: MapInteger(0), // staged_holons.len() as i64,
        saved_holons: Vec::new(),
        abandoned_holons: Vec::new(),
    };

    // Get the staged holons from the Nursery
    let stage_count = MapInteger(staged_holons.len() as i64);
    if stage_count.0 < 1 {
        warn!("Stage empty, nothing to commit!");
        return Ok(response);
    }
    response.commits_attempted = stage_count;

    // FIRST PASS: Commit Staged Holons
    {
        info!("\n\nStarting FIRST PASS... commit staged_holons...");
        for rc_holon in staged_holons.iter().cloned() {
            {
                rc_holon.read().unwrap().is_accessible(AccessType::Commit)?;
            }

            let should_commit = {
                let borrowed = rc_holon.read().unwrap();
                matches!(&*borrowed, Holon::Staged(_))
            };

            if should_commit {
                trace!(" In commit_service... getting ready to call commit()");
                let outcome = commit_holon(rc_holon.clone());
                match outcome {
                    Ok(holon) => match holon {
                        Holon::Staged(ref staged_holon) => {
                            match staged_holon.get_staged_state() {
                                StagedState::Abandoned => {
                                    // should these be indexed?
                                    //if !response.abandoned_holons.contains(&holon) {
                                    response.abandoned_holons.push(holon);
                                    //}
                                }
                                StagedState::Committed(_saved_id) => {
                                    response.saved_holons.push(holon);
                                }
                                _ => {}
                            }
                        }
                        _ => unreachable!(),
                    },
                    Err(error) => {
                        response.status = CommitRequestStatus::Incomplete;
                        warn!("Attempt to commit holon returned error: {:?}", error.to_string());
                    }
                };
            } else {
                return Err(HolonError::InvalidHolonReference(
                    "All holons in staged_holons must be of type Holon::Staged".to_string(),
                ));
            }
        }
    }

    if response.status == CommitRequestStatus::Incomplete {
        return Ok(response);
    }

    //  SECOND PASS: Commit relationships
    {
        info!("\n\nStarting 2ND PASS... commit relationships for the saved staged_holons...");
        //let commit_manager = context.commit_manager.borrow();
        for rc_holon in staged_holons {
            let mut holon_write = rc_holon.write().unwrap();

            if let Holon::Staged(staged_holon) = &mut *holon_write {
                let outcome = commit_relationships(context, &staged_holon);
                if let Err(error) = outcome {
                    staged_holon.add_error(error.clone())?;
                    response.status = CommitRequestStatus::Incomplete;
                    warn!("Attempt to commit relationship returned error: {:?}", error.to_string());
                }
            }
        }
    }

    info!("\n\n VVVVVVVVVVV   SAVED HOLONS AFTER COMMIT VVVVVVVVV\n");
    // for saved_holon in &response.saved_holons {
    //     debug!("{}", as_json(saved_holon));
    // }
    Ok(response)
}

/// `commit_holon` saves a staged holon to the persistent store.
///
/// If the staged_state is `Abandoned`, 'Committed', or 'ForUpdate' then commit does nothing.
///
/// If the staged holon is `ForCreate`, commit attempts to create a HolonNode.
///
/// If the staged holon is `ForUpdateChanged`, commit persists a new version of the HolonNode.
///
/// If the create or update is successful, the holon's `LocalId` is set from the action_address of the Record
/// returned, its `staged_state` is changed to `Committed`, so that commits are idempotent, and the
/// function returns a clone of the holon_write.
///
/// If an error is encountered, it is pushed into the holons `errors` vector, the holon's state
/// is left unchanged and an Err is returned.
///
fn commit_holon(rc_holon: Arc<RwLock<Holon>>) -> Result<Holon, HolonError> {
    let mut holon_write = rc_holon.write().unwrap();
    if let Holon::Staged(staged_holon) = &mut *holon_write {
        let staged_state = staged_holon.get_staged_state();

        match staged_state {
            StagedState::ForCreate => {
                // Create a new HolonNode from this Holon and request it be created
                trace!("StagedState is New... requesting new HolonNode be created in the DHT");
                let node = staged_holon.into_node_model();
                let result = create_holon_node(HolonNode::from(node));

                match result {
                    Ok(record) => {
                        staged_holon
                            .to_committed(LocalId(record.action_address().clone().into_inner()))?;

                        return Ok(holon_write.clone());
                    }
                    Err(error) => {
                        let holon_error = holon_error_from_wasm_error(error);
                        staged_holon.add_error(holon_error.clone())?;

                        return Err(holon_error);
                    }
                }
            }
            StagedState::ForUpdateChanged => {
                // Changed holons MUST have an original_id
                let original_id = staged_holon.get_original_id();
                if let Some(id) = original_id {
                    let original_holon_node_hash = try_action_hash_from_local_id(&id)?;
                    let previous_holon_node_hash =
                        try_action_hash_from_local_id(&staged_holon.get_local_id()?)?;

                    let input = UpdateHolonNodeInput {
                        original_holon_node_hash,
                        previous_holon_node_hash,
                        updated_holon_node: HolonNode::from(staged_holon.clone().into_node_model()),
                    };
                    debug!("Requesting HolonNode be updated in the DHT"); //

                    let result = update_holon_node(input);
                    match result {
                        Ok(record) => {
                            staged_holon.to_committed(local_id_from_action_hash(
                                record.action_address().clone(),
                            ))?;

                            return Ok(holon_write.clone());
                        }
                        Err(error) => {
                            let holon_error = holon_error_from_wasm_error(error);
                            staged_holon.add_error(holon_error.clone())?;

                            return Err(holon_error);
                        }
                    }
                } else {
                    let holon_error = HolonError::HolonNotFound(
                        "Holon marked Changed, but has no record".to_string(),
                    );
                    staged_holon.add_error(holon_error.clone())?;

                    return Err(holon_error);
                }
            }
            _ => {
                // No save needed for Abandoned, Committed, or ForUpdate, just return Holon
                debug!("Skipping commit for holon in {:#?} state", staged_state);

                Ok(holon_write.clone())
            }
        }
    } else {
        Err(HolonError::InvalidParameter(format!(
            "Can only commit staged holons, attempted to commit: {:?} ",
            holon_write
        )))
    }
}

/// commit_relationship() saves a `Saved` holon's relationships as SmartLinks. It should only be invoked
/// AFTER staged_holons have been successfully committed, thus only accepts a StagedHolon object.
///
/// If the staged_state is `Committed`, commit_relationship iterates through the holon's
/// `relationship_map` and calls commit on each member's HolonCollection.
/// Any other states are ignored.
///
/// The function only returns OK if ALL commits are successfull.
fn commit_relationships(
    context: &dyn HolonsContextBehavior,
    holon: &StagedHolon,
) -> Result<(), HolonError> {
    debug!("Entered Holon::commit_relationships");

    match holon.get_staged_state() {
        StagedState::Committed(local_id) => {
            for (name, holon_collection_rc) in holon.get_staged_relationship_map()?.map.iter() {
                debug!("COMMITTING {:#?} relationship", name.0.clone());
                let holon_collection = holon_collection_rc.read().unwrap();
                commit_relationship(context, local_id.clone(), name.clone(), &holon_collection)?;
            }

            Ok(())
        }
        _ => {
            // Ignore all other states, just return Ok
            Ok(())
        }
    }
}

/// The method
fn commit_relationship(
    context: &dyn HolonsContextBehavior,
    source_id: LocalId,
    name: RelationshipName,
    collection: &HolonCollection,
) -> Result<(), HolonError> {
    collection.is_accessible(AccessType::Commit)?;

    save_smartlinks_for_collection(context, source_id.clone(), name.clone(), collection)?;

    Ok(())
}

/// This method creates smartlinks from the specified source_id for the specified relationship name
/// to each holon its collection that has a holon_id.
fn save_smartlinks_for_collection(
    context: &dyn HolonsContextBehavior,
    source_id: LocalId,
    name: RelationshipName,
    collection: &HolonCollection,
) -> Result<(), HolonError> {
    info!(
        "Calling commit on each HOLON_REFERENCE in the collection for [source_id {:#?}]->{:#?}.",
        source_id,
        name.0 .0.clone()
    );
    for holon_reference in collection.get_members() {
        // Only commit references to holons with id's (i.e., Saved)
        if let Ok(target_id) = holon_reference.holon_id(context) {
            let key_option = holon_reference.key(context)?;
            let smartlink: SmartLink = if let Some(key) = key_option {
                let mut prop_vals: PropertyMap = BTreeMap::new();
                prop_vals.insert(
                    PropertyName(MapString("key".to_string())),
                    BaseValue::StringValue(key),
                );
                SmartLink {
                    from_address: source_id.clone(),
                    to_address: target_id,
                    relationship_name: name.clone(),
                    smart_property_values: Some(prop_vals),
                }
            } else {
                SmartLink {
                    from_address: source_id.clone(),
                    to_address: target_id,
                    relationship_name: name.clone(),
                    smart_property_values: None,
                }
            };
            debug!("saving smartlink: {:#?}", smartlink);
            save_smartlink(smartlink)?;
        } else {
            warn!("Tried to commit target : {:#?} without HolonId", holon_reference);
        }
    }
    Ok(())
}
