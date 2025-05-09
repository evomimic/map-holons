use hdk::prelude::*;

// use crate::{
//     create_holon_node, save_smartlink, update_holon_node, SmartLink, UpdateHolonNodeInput,
// };
use crate::guest_shared_objects::{save_smartlink, SmartLink};
use crate::persistence_layer::{create_holon_node, update_holon_node, UpdateHolonNodeInput};

use holons_core::core_shared_objects::{
    AccessType, CommitRequestStatus, CommitResponse, Holon, HolonCollection, HolonError,
    HolonState, RelationshipName,
};
use holons_core::reference_layer::{HolonReadable, HolonsContextBehavior};
use holons_core::utils::as_json;
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{LocalId, PropertyMap, PropertyName};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// `commit`
///
/// This function attempts to persist the state of all staged_holons AND their relationships.
///
/// The commit is performed in two passes: (1) staged_holons, (2) their relationships.
///
/// In the first pass,
/// * if a staged_holon commit succeeds,
///     * change holon's state to `Saved`
///     * populate holon's saved_node
///     * add the holon to the saved_nodes vector in the CommitResponse
/// * if a staged_holon commit fails,
///     * leave holon's state unchanged
///     * leave holon's saved_node unpopulated
///     * push the error into the holon's errors vector
///     * do NOT add the holon to the saved_nodes vector in the CommitResponse
///
/// If ANY staged_holon commit fails:
/// * The 2nd pass (to commit the staged_holon's relationships) is SKIPPED
/// * the overall return status in the CommitResponse is set to `Incomplete`
/// * the function returns.
///
/// Otherwise, the 2nd pass is performed.
/// * If ANY attempt to add a relationship generates an Error, the error is pushed into the
/// source holon's `errors` vector and processing continues
///
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
    staged_holons: &Vec<Rc<RefCell<Holon>>>,
) -> Result<CommitResponse, HolonError> {
    debug!("Entering commit...");

    // Initialize the request_status to Complete, assuming all commits will succeed
    // If any commit errors are encountered, reset request_status to `Incomplete`
    let mut response = CommitResponse {
        status: CommitRequestStatus::Complete,
        commits_attempted: MapInteger(0), // staged_holons.len() as i64),
        saved_holons: Vec::new(),
        abandoned_holons: Vec::new(),
    };

    // Get the staged holons from the Nursery

    let stage_count = MapInteger(staged_holons.len() as i64);
    if stage_count.0 < 1 {
        info!("Stage empty, nothing to commit!");
        return Ok(response);
    }
    response.commits_attempted = stage_count;

    // FIRST PASS: Commit Staged Holons
    {
        info!("\n\nStarting FIRST PASS... commit staged_holons...");
        for rc_holon in staged_holons {
            trace!(" In commit_service... getting ready to call commit()");
            let outcome = commit_holon(rc_holon);
            match outcome {
                Ok(holon) => match holon.state {
                    HolonState::Abandoned => {
                        // should these be indexed?
                        //if !response.abandoned_holons.contains(&holon) {
                        response.abandoned_holons.push(holon);
                        //}
                    }
                    HolonState::Saved => {
                        response.saved_holons.push(holon);
                    }
                    _ => {}
                },
                Err(error) => {
                    response.status = CommitRequestStatus::Incomplete;
                    warn!("Attempt to commit holon returned error: {:?}", error.to_string());
                }
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
            //commit_manager.staged_holons.clone
            let outcome = commit_relationships(context, rc_holon);
            if let Err(error) = outcome {
                rc_holon.borrow_mut().errors.push(error.clone());
                response.status = CommitRequestStatus::Incomplete;
                warn!("Attempt to commit relationship returned error: {:?}", error.to_string());
            }
        }
    }

    info!("\n\n VVVVVVVVVVV   SAVED HOLONS AFTER COMMIT VVVVVVVVV\n");
    for saved_holon in &response.saved_holons {
        debug!("{}", as_json(saved_holon));
    }
    Ok(response)
}

/// `commit_holon` saves a staged holon to the persistent store.
///
/// If the staged holon is already  `Fetched`, `Saved`, or `Abandoned`, commit does nothing.
///
/// If the staged holon is `New`, commit attempts to create a HolonNode.
///
/// If the staged holon is `Changed`, commit persists a new version of the HolonNode
///
/// If the create or update is successful, the holon's `saved_node` is set from the record
/// returned, its `state` is changed to `Saved`, so that commits are idempotent, and the
/// function returns a clone of self.
///
/// If an error is encountered, it is pushed into the holons `errors` vector, the holon's state
/// is left unchanged and an Err is returned.
///

fn commit_holon(rc_holon: &Rc<RefCell<Holon>>) -> Result<Holon, HolonError> {
    let mut holon_write = rc_holon.borrow_mut();
    debug!(
        "Entered Holon::commit for holon with key {:#?} in {:#?} state",
        holon_write.get_key()?.unwrap_or_else(|| MapString("<None>".to_string())).0,
        holon_write.state
    );
    match holon_write.state {
        HolonState::New => {
            // Create a new HolonNode from this Holon and request it be created
            trace!("HolonState is New... requesting new HolonNode be created in the DHT");
            let result = create_holon_node(holon_write.clone().into_node());

            match result {
                Ok(record) => {
                    holon_write.state = HolonState::Saved;
                    holon_write.saved_node = Option::from(record);

                    Ok(holon_write.clone())
                }
                Err(error) => {
                    let holon_error = HolonError::from(error);
                    holon_write.errors.push(holon_error.clone());
                    Err(holon_error)
                }
            }
        }

        HolonState::Changed => {
            // Changed holons MUST have an original_id
            if let Some(ref node) = holon_write.saved_node {
                let original_holon_node_hash = match holon_write.get_original_id()? {
                    Some(id) => Ok(id.0),
                    None => Err(HolonError::InvalidUpdate("original_id".to_string())),
                }?;
                let input = UpdateHolonNodeInput {
                    original_holon_node_hash,
                    previous_holon_node_hash: node.action_address().clone(),
                    updated_holon_node: holon_write.clone().into_node(),
                };
                debug!("Requesting HolonNode be updated in the DHT");
                let result = update_holon_node(input);
                match result {
                    Ok(record) => {
                        holon_write.state = HolonState::Saved;
                        holon_write.saved_node = Option::from(record);
                        Ok(holon_write.clone())
                    }
                    Err(error) => {
                        let holon_error = HolonError::from(error);
                        holon_write.errors.push(holon_error.clone());
                        Err(holon_error)
                    }
                }
            } else {
                let holon_error = HolonError::HolonNotFound(
                    "Holon marked Changed, but has no saved_node".to_string(),
                );
                holon_write.errors.push(holon_error.clone());
                Err(holon_error)
            }
        }

        _ => {
            // No save needed for Fetched, Saved, Abandoned, or Transient, just return Holon
            debug!("Skipping commit for holon in {:#?} state", holon_write.state);

            Ok(holon_write.clone())
        }
    }
}
/// commit_relationship() saves a `Saved` holon's relationships as SmartLinks. It should only be invoked
/// AFTER staged_holons have been successfully committed.
///
/// If the staged holon is `Fetched`, `New`, or `Changed` commit does nothing.
///
/// If the staged holon is `Saved`, commit_relationship iterates through the holon's
/// `relationship_map` and calls commit on each member's HolonCollection.
///
/// If all commits are successful, the function returns a clone a self. Otherwise, the
/// function returns an error.
///
fn commit_relationships(
    context: &dyn HolonsContextBehavior,
    rc_holon: &Rc<RefCell<Holon>>,
) -> Result<Holon, HolonError> {
    let holon = rc_holon.borrow();
    debug!("Entered Holon::commit_relationships");

    match holon.state {
        HolonState::Saved => {
            match holon.saved_node.clone() {
                Some(record) => {
                    let source_local_id = LocalId(record.action_address().clone());
                    // Use the public `iter()` method to access the map
                    for (name, holon_collection_rc) in holon.staged_relationship_map.iter() {
                        debug!("COMMITTING {:#?} relationship", name.0.clone());
                        // Borrow the `RefCell` to access the `HolonCollection`
                        let holon_collection = holon_collection_rc.borrow();
                        commit_relationship(
                            context,
                            source_local_id.clone(),
                            name.clone(),
                            &holon_collection,
                        )?;
                    }

                    Ok(holon.clone())
                }
                None => Err(HolonError::HolonNotFound(
                    "Holon marked Saved, but has no saved_node".to_string(),
                )),
            }
        }

        _ => {
            // Ignore all other states, just return self
            Ok(holon.clone())
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
        if let Ok(target_id) = holon_reference.get_holon_id(context) {
            let key_option = holon_reference.get_key(context)?;
            let smartlink: SmartLink = if let Some(key) = key_option {
                let mut prop_vals: PropertyMap = BTreeMap::new();
                prop_vals.insert(
                    PropertyName(MapString("key".to_string())),
                    Some(BaseValue::StringValue(key)),
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
