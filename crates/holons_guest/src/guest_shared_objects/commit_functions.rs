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
        Holon, HolonCollection, ReadableHolonState, StagedHolon,
    },
    new_holon,
    reference_layer::{HolonsContextBehavior, ReadableHolon},
    HolonReference, StagedReference, WritableHolon,
};

use base_types::{BaseValue, MapString};
use core_types::HolonError;
use holons_core::core_shared_objects::holon_pool::StagedHolonPool;
use holons_core::reference_layer::TransientReference;
use integrity_core_types::{LocalId, PropertyMap, RelationshipName};
use type_names::CorePropertyTypeName::Key;
pub use type_names::CorePropertyTypeName::{CommitRequestStatus, CommitsAttempted};
pub use type_names::CoreRelationshipTypeName::{HolonsAbandoned, HolonsCommitted};
pub use type_names::{
    CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName, CoreValueTypeName,
    ToPropertyName, ToRelationshipName,
};

//// Represents the result of attempting to commit a staged holon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitOutcome {
    /// The holon was successfully persisted (created or updated).
    Saved,
    /// The holon was explicitly marked as abandoned and skipped.
    Abandoned,
    /// No persistence action was required (already committed or unchanged).
    NoAction,
}

/// `commit`
///
/// This function attempts to persist the state of all staged_holons AND their relationships.
/// It is not completely Atomic - since successfully committed holons will get saved to the Holochain persistence store,
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
    holon_pool: &Arc<RwLock<StagedHolonPool>>,
) -> Result<TransientReference, HolonError> {
    info!("Entering commit...");

    // Initialize the request_status to Complete, assuming all commits will succeed
    // If any commit errors are encountered, reset request_status to `Incomplete`
    // let mut response = CommitResponse {
    //     status: CommitRequestStatus::Complete,
    //     commits_attempted: MapInteger(0), // staged_holons.len() as i64,
    //     saved_holons: Vec::new(),
    //     abandoned_holons: Vec::new(),
    // };

    // Get the staged holons count from the HolonPool
    let stage_count = holon_pool.read().unwrap().len() as i64;

    let mut response_reference =
        new_holon(context, Some(MapString("Commit Response".to_string())))?;
    response_reference
        .with_property_value(context, CommitRequestStatus, "Complete")?
        .with_property_value(context, CommitsAttempted, stage_count)?;

    if stage_count < 1 {
        info!("Stage empty, nothing to commit!");
        return Ok(response_reference);
    }

    // === FIRST PASS: Commit Staged Holons ===
    {
        info!("\n\nStarting FIRST PASS... commit staged holons...");

        let staged_references = {
            let pool = holon_pool.read().unwrap();
            pool.get_staged_references()
        };

        let mut saved_holons: Vec<HolonReference> = Vec::new();
        let mut abandoned_holons: Vec<HolonReference> = Vec::new();

        for staged_reference in staged_references {
            staged_reference.is_accessible(context, AccessType::Commit)?;

            trace!("Committing {:?}", staged_reference.temporary_id());

            match commit_holon(&staged_reference, context) {
                Ok(CommitOutcome::Saved) => {
                    let holon_id = staged_reference.holon_id(context)?;
                    let key_string: MapString =
                        staged_reference.key(context)?.ok_or_else(|| {
                            HolonError::HolonNotFound("Committed holon has no key".into())
                        })?;
                    let saved_reference = HolonReference::smart_with_key(holon_id, key_string);
                    saved_holons.push(saved_reference);
                }
                Ok(CommitOutcome::Abandoned) => {
                    abandoned_holons.push((&staged_reference).into());
                }
                Ok(CommitOutcome::NoAction) => {
                    trace!("No action required for {:?}", staged_reference.temporary_id());
                }
                Err(error) => {
                    response_reference.with_property_value(
                        context,
                        CommitRequestStatus,
                        "Incomplete",
                    )?;
                    abandoned_holons.push((&staged_reference).into());
                    warn!("Commit failed for {:?}: {:?}", staged_reference.temporary_id(), error);
                }
            }
        }

        // Attach results to the CommitResponse holon
        response_reference.add_related_holons(context, HolonsCommitted, saved_holons)?;
        response_reference.add_related_holons(context, HolonsAbandoned, abandoned_holons)?;
    }

    // Check if Pass 1 ended with an incomplete status
    if let Some(status_value) = response_reference.property_value(context, "CommitRequestStatus")? {
        let status_string: String = (&status_value).into();
        if status_string == "Incomplete" {
            info!("Commit Pass 1 incomplete — skipping Pass 2.");
            return Ok(response_reference);
        }
    }

    //  SECOND PASS: Commit relationships
    // We can iterate all staged references again; `commit_relationships` is a no-op
    // unless the holon is in `StagedState::Committed(_)`.
    let staged_references = {
        let pool = holon_pool.read().unwrap();
        pool.get_staged_references()
    };

    for staged_reference in staged_references {
        // Resolve the holon and take a write lock so we can attach an error if needed
        let rc_holon = staged_reference.get_holon_to_commit(context)?;
        let mut holon_write = rc_holon.write().unwrap();

        if let Holon::Staged(staged_holon) = &mut *holon_write {
            if let Err(error) = commit_relationships(context, staged_holon) {
                // Record the error on the holon and mark the overall response incomplete
                staged_holon.add_error(error.clone())?;
                response_reference.with_property_value(
                    context,
                    "CommitRequestStatus",
                    "Incomplete",
                )?;

                warn!(
                    "Attempt to commit relationships failed for {:?}: {:?}",
                    staged_reference.temporary_id(),
                    error
                );
            }
        }
    }

    info!("\n\n VVVVVVVVVVV   SAVED HOLONS AFTER COMMIT VVVVVVVVV\n");
    // Optionally dump here if you have a helper like `as_json` for references/ids.

    // Done — return the CommitResponse holon reference
    Ok(response_reference)
}

/// Attempts to persist the holon referenced by the given [`StagedReference`].
///
/// This low-level persistence routine determines the holon's current [`StagedState`]
/// and performs the corresponding create or update operation in the DHT. The holon
/// is mutated in place to reflect its new committed state.
///
/// Returns:
/// * `Ok(Saved)` – Holon successfully created or updated.
/// * `Ok(Abandoned)` – Holon was explicitly marked abandoned and skipped.
/// * `Ok(NoAction)` – Holon required no persistence action.
/// * `Err(HolonError)` – Persistence failure or invalid state.
///
/// This function is only invoked from within the guest environment. It is safe and
/// idempotent to call repeatedly; holons already committed or abandoned are skipped.
fn commit_holon(
    staged_reference: &StagedReference,
    context: &dyn HolonsContextBehavior,
) -> Result<CommitOutcome, HolonError> {
    // Resolve the staged holon from the pool
    let rc_holon = staged_reference.get_holon_to_commit(context)?;
    let mut holon_write = rc_holon.write().unwrap();

    if let Holon::Staged(staged_holon) = &mut *holon_write {
        let staged_state = staged_holon.get_staged_state();

        match staged_state {
            // === CREATE NEW NODE ============================================================
            StagedState::ForCreate => {
                trace!("StagedState::ForCreate — creating HolonNode in DHT");
                let node = staged_holon.into_node_model();
                let record = create_holon_node(HolonNode::from(node))
                    .map_err(holon_error_from_wasm_error)?;

                staged_holon.to_committed(LocalId(record.action_address().clone().into_inner()))?;
                Ok(CommitOutcome::Saved)
            }

            // === UPDATE EXISTING NODE =======================================================
            StagedState::ForUpdateChanged => {
                trace!("StagedState::ForUpdateChanged — updating HolonNode in DHT");

                if let Some(original_id) = staged_holon.original_id() {
                    let original_hash = try_action_hash_from_local_id(&original_id)?;
                    let previous_hash =
                        try_action_hash_from_local_id(&staged_holon.get_local_id()?)?;

                    let input = UpdateHolonNodeInput {
                        original_holon_node_hash: original_hash,
                        previous_holon_node_hash: previous_hash,
                        updated_holon_node: HolonNode::from(staged_holon.clone().into_node_model()),
                    };

                    let record = update_holon_node(input).map_err(holon_error_from_wasm_error)?;

                    staged_holon
                        .to_committed(local_id_from_action_hash(record.action_address().clone()))?;
                    Ok(CommitOutcome::Saved)
                } else {
                    let holon_error = HolonError::HolonNotFound(
                        "Holon marked Changed but has no record".to_string(),
                    );
                    staged_holon.add_error(holon_error.clone())?;
                    Err(holon_error)
                }
            }

            // === ABANDONED HOLON ============================================================
            StagedState::Abandoned => {
                debug!("Skipping commit for Abandoned holon.");
                Ok(CommitOutcome::Abandoned)
            }

            // === ALREADY COMMITTED OR NO-OP ================================================
            StagedState::Committed(_) | StagedState::ForUpdate => {
                debug!(
                    "Skipping commit for holon in {:?} state (no action required)",
                    staged_state
                );
                Ok(CommitOutcome::NoAction)
            }
        }
    } else {
        Err(HolonError::InvalidParameter(format!(
            "Can only commit staged holons, attempted to commit: {:?}",
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
/// The function only returns OK if ALL commits are successful.
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
/// to each holon in its collection that has a holon_id.
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

    let key_prop = CorePropertyTypeName::Key.as_property_name();

    for holon_reference in collection.get_members() {
        // Only commit references to holons with ids (i.e., Saved)
        if let Ok(target_id) = holon_reference.holon_id(context) {
            let key_option = holon_reference.key(context)?;
            let smartlink: SmartLink = if let Some(key) = key_option {
                let mut prop_vals: PropertyMap = BTreeMap::new();
                prop_vals.insert(key_prop.clone(), BaseValue::StringValue(key));
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
