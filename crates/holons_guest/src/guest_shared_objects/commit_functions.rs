use std::collections::BTreeMap;

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
use holons_core::reference_layer::TransientReference;
use integrity_core_types::{LocalId, PropertyMap, RelationshipName};

pub use type_names::CorePropertyTypeName::{CommitRequestStatus, CommitsAttempted};
pub use type_names::CoreRelationshipTypeName::{AbandonedHolons, SavedHolons};
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
/// Executes a two-pass commit of all staged holons and their relationships,
/// returning a **`TransientReference` to a CommitResponseType holon** that
/// reports the outcome of the operation.
///
/// ## What This Function Produces
///
/// Instead of returning a Rust `CommitResponse` struct (the old behavior),
/// this function now **constructs a CommitResponse holon instance** using the
/// holon operations API.
///
/// The returned `TransientReference` points to a `CommitResponseType` holon with:
/// - **Property** `CommitRequestStatus` = `"Complete"` or `"Incomplete"`
/// - **Property** `CommitsAttempted` = number of staged holons
/// - **Relationship** `SavedHolons` → all successfully committed holons
/// - **Relationship** `AbandonedHolons` → all holons skipped or failed
///
/// This holon is a first-class MAP holon and can be returned directly as a
/// dance response or inspected by follow-up guest or client processes.
///
/// ## Process Overview
///
/// The commit proceeds in **two passes**:
///
/// ### **Pass 1 — Commit staged holons**
///
/// Each staged holon is processed according to its `StagedState`:
///
/// - **ForCreate**
///   Persist as a new HolonNode; update the staged holon to `Committed(saved_id)`
///
/// - **ForUpdateChanged**
///   Update the existing HolonNode; update to `Committed(saved_id)`
///
/// - **Abandoned**
///   Skipped and added to the `AbandonedHolons` relationship
///
/// - **ForUpdate / Already Committed**
///   No-op
///
/// Any holon that fails to commit:
/// - keeps its original staged state
/// - receives its error in the holon’s internal error list
/// - causes the CommitResponse holon’s `CommitRequestStatus` to be set to `"Incomplete"`
///
/// If **any** holon fails in pass 1, the function returns immediately after
/// building the response holon. **Pass 2 is skipped.**
///
/// ### **Pass 2 — Commit relationships**
///
/// Only executed if pass 1 completes with no failures.
///
/// For each staged holon in `Committed(saved_id)` state:
/// - iterate all staged relationship collections
/// - create SmartLinks for each member (if the target holon has a LocalId)
/// - record any errors into the source staged holon
/// - update the CommitResponse holon to `"Incomplete"` if any error occurs
///
/// If all relationships succeed, the overall result remains `"Complete"`.
///
/// ## Return Value
///
/// Returns:
/// Ok(TransientReference)
/// pointing to the CommitResponse holon created during the process.
///
/// This holon is always returned—even if the commit is partially successful—
/// and contains a complete summary of saved and abandoned items.
///
/// ## Clearing Staged Holons
///
/// If both passes succeed (no `Incomplete` status), the guest holon service
/// is responsible for clearing the staged holon pool afterward.
pub fn commit(
    context: &dyn HolonsContextBehavior,
    staged_references: &[StagedReference],
) -> Result<TransientReference, HolonError> {
    info!("Entering commit...");

    // Number of staged holons is derived from the provided references.
    let stage_count = staged_references.len() as i64;

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

        let mut saved_holons: Vec<HolonReference> = Vec::new();
        let mut abandoned_holons: Vec<HolonReference> = Vec::new();

        for staged_reference in staged_references {
            staged_reference.is_accessible(context, AccessType::Commit)?;

            trace!("Committing {:?}", staged_reference.temporary_id());

            match commit_holon(staged_reference, context) {
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
                    // StagedReference → HolonReference via From<&StagedReference>
                    abandoned_holons.push(staged_reference.into());
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
                    abandoned_holons.push(staged_reference.into());
                    warn!("Commit failed for {:?}: {:?}", staged_reference.temporary_id(), error);
                }
            }
        }

        // Attach results to the CommitResponse holon
        response_reference.add_related_holons(context, SavedHolons, saved_holons)?;
        response_reference.add_related_holons(context, AbandonedHolons, abandoned_holons)?;
    }

    // Check if Pass 1 ended with an incomplete status
    if let Some(status_value) = response_reference.property_value(context, CommitRequestStatus)? {
        let status_string: String = (&status_value).into();
        if status_string == "Incomplete" {
            info!("Commit Pass 1 incomplete — skipping Pass 2.");
            return Ok(response_reference);
        }
    }

    // === SECOND PASS: Commit relationships ===
    //
    // We can iterate all staged references again; `commit_relationships` is a no-op
    // unless the holon is in `StagedState::Committed(_)`.
    for staged_reference in staged_references {
        // Resolve the holon and take a write lock so we can attach an error if needed
        let rc_holon = staged_reference.get_holon_to_commit(context)?;
        let mut holon_write = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon during relationship commit: {}",
                e
            ))
        })?;

        if let Holon::Staged(staged_holon) = &mut *holon_write {
            if let Err(error) = commit_relationships(context, staged_holon) {
                // Record the error on the holon and mark the overall response incomplete
                staged_holon.add_error(error.clone())?;
                response_reference.with_property_value(
                    context,
                    CommitRequestStatus,
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
    let mut holon_write = rc_holon.write().map_err(|e| {
        HolonError::FailedToAcquireLock(format!(
            "Failed to acquire write lock on staged holon during commit: {}",
            e
        ))
    })?;

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
                let holon_collection = holon_collection_rc.read().map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire read lock on relationship collection for {}: {}",
                        name.0 .0, e
                    ))
                })?;
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

    let members = collection.get_members();
    debug!("Relationship {:?} has {} members to commit", name.0 .0, members.len());

    for (idx, holon_reference) in members.iter().enumerate() {
        debug!("Target index={} holon_reference={:#?}", idx, holon_reference);

        // 1) Narrow down: do we get through holon_id?
        let target_id = match holon_reference.holon_id(context) {
            Ok(id) => {
                debug!("Resolved holon_id for index {}: {:?}", idx, id);
                id
            }
            Err(err) => {
                warn!(
                    "Failed to get holon_id for relationship {:?} at index {}: {:?}",
                    name.0 .0, idx, err
                );
                continue;
            }
        };

        // 2) Narrow down: do we get through key()?
        let key_option = match holon_reference.key(context) {
            Ok(k) => {
                debug!("Resolved key for index {}: {:?}", idx, k);
                k
            }
            Err(err) => {
                error!(
                    "Error getting key for relationship {:?} at index {}: {:?}",
                    name.0 .0, idx, err
                );
                return Err(err);
            }
        };

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

        debug!("saving smartlink (idx={}): {:#?}", idx, smartlink);
        save_smartlink(smartlink)?;
    }

    Ok(())
}
