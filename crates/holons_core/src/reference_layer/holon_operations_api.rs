//! holon_operations_api.rs
//!
//! This module provides a higher-level API for performing operations on holons,
//! such as staging, committing, and deleting. It abstracts away the complexity
//! of retrieving and interacting with the underlying services managed by the
//! `HolonSpaceManager`.
//!
//! By providing a friendly interface, this API simplifies access to the
//! following functionality:
//! - Staging holons (via `HolonStagingBehavior`)
//! - Committing changes (via `HolonServiceApi`)
//! - Deleting holons (via `HolonServiceApi`)
//!
//! This API is designed to complement the lower-level, method-based APIs
//! available in:
//! - `Holon`: For working with individual holons.
//! - `HolonCollection`: For working with collections of holons.
//!
//! ### Purpose
//! The functions in this module serve as the "glue" that bridges the higher-level
//! application logic with the lower-level holon services, hiding service lookups
//! and improving usability.

use crate::core_shared_objects::{CommitResponse, Holon, HolonBehavior};
use crate::reference_layer::TransientReference;
use crate::{
    HolonCollection, HolonReference, HolonsContextBehavior, SmartReference, StagedReference,
};
use base_types::MapString;
use core_types::{HolonError, LocalId, PropertyMap, PropertyName};
// Import traits for transient and staging behaviors and borrow utilities
use crate::reference_layer::holon_staging_behavior::HolonStagingBehavior;
use crate::reference_layer::transient_holon_behavior::TransientHolonBehavior;

//TODO: move static/stateless HDI/HDK functions to the Holon_service

/// Commits the state of all staged holons and their relationships to the DHT.
///
/// This function attempts to persist the state of all staged holons and their relationships
/// to the distributed hash table (DHT). It can be called from both the client-side and the
/// guest-side:
/// - **On the client-side**: The call is delegated to the guest-side for execution, where the
///   actual DHT operations are performed.
/// - **On the guest-side**: The commit process interacts directly with the DHT.
///
/// The function returns either a `HolonError` (indicating a system-level failure) or a
/// `CommitResponse`. If a `CommitResponse` is returned, it will indicate whether the commit
/// was fully successful (`Complete`) or partially successful (`Incomplete`).
///
/// # Commit Outcomes
///
/// ## Complete Commit
/// If the commit process fully succeeds:
/// - The `CommitResponse` will have a `Complete` status.
/// - All staged holons and their relationships are successfully persisted to the DHT.
/// - The `CommitResponse` includes a list of all successfully saved holons, with their `record`
/// (including their `LocalId`) populated.
/// - The `space_manager`'s list of staged holons is cleared.
///
/// ## Partial Commit
/// If the commit process partially succeeds:
/// - The `CommitResponse` will have an `Incomplete` status.
/// - **No staged holons are removed** from the `space_manager`.
/// - Holons that were successfully committed:
///     - Have their state updated to `Saved`.
///     - Include their saved node (indicating they were persisted).
///     - Are added to the `CommitResponse`'s `records` list.
/// - Holons that were **not successfully committed**:
///     - Retain their previous state (unchanged).
///     - Have their `errors` vector populated with the errors encountered during the commit.
///     - Do **not** include a saved node.
///     - Are **not** added to the `CommitResponse`'s `records` list.
/// - Correctable errors in the `errors` vector allow the `commit` call to be retried until the
///   process succeeds completely.
///
/// ## Failure
/// If the commit process fails entirely due to a system-level issue:
/// - The function returns a `HolonError`.
/// - No changes are made to the staged holons.
///
/// # Arguments
/// - `context`: The context to retrieve holon services.
///
/// # Returns
/// - `Ok(CommitResponse)`:
///     - If the commit process is successful (either completely or partially).
///     - Use the `CommitResponse`'s status to determine whether the commit is `Complete` or `Incomplete`.
/// - `Err(HolonError)`:
///     - If a system-level failure prevents the commit process from proceeding.
///
/// # Errors
/// - Returns a `HolonError` if the commit operation encounters a system-level issue.
///

pub fn commit(context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError> {
    let holon_service = context.get_space_manager().get_holon_service();
    let commit_response = holon_service.commit_internal(context)?;

    Ok(commit_response)
}

/// Creates a new TransientHolon and assigns the specified key
/// Returns a TransientReference to the newly created holon
pub fn new_holon(
    context: &dyn HolonsContextBehavior,
    key: MapString,
) -> Result<TransientReference, HolonError> {
    let transient_service = context.get_space_manager().get_transient_behavior_service();
    let transient_reference = transient_service
        .read()
        .map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on transient_behavior_service: {}",
                e
            ))
        })?
        .create_empty(key)?;

    Ok(transient_reference)
}

/// Deletes a holon identified by its ID.
///
/// This function removes a holon from the holon space and can be called from either the client-side
/// or the guest-side:
/// - **On the client-side**: The call is delegated to the guest-side for execution, where the actual
///   deletion operations (e.g., removing entries from the DHT) are performed.
/// - **On the guest-side**: The deletion operations are performed directly.
///
/// Regardless of the environment, the net effect is the same: the specified holon is deleted.
///
/// # Arguments
/// - `context`: The context to retrieve holon services.
/// - `local_id`: The ID of the holon to delete. Note ONLY local holons may be deleted.
///
/// # Returns
/// - `Ok(())` if the holon is successfully deleted.
/// - `Err(HolonError)` if the deletion fails.
///
/// # Errors
/// - Returns a `HolonError` if the specified holon cannot be found or deleted.
///
pub fn delete_holon(
    context: &dyn HolonsContextBehavior,
    local_id: LocalId,
) -> Result<(), HolonError> {
    let holon_service = context.get_space_manager().get_holon_service();
    holon_service.delete_holon_internal(&local_id)
}

// == GETTERS == //

pub fn get_all_holons(context: &dyn HolonsContextBehavior) -> Result<HolonCollection, HolonError> {
    let holon_service = context.get_space_manager().get_holon_service();
    holon_service.get_all_holons_internal(context)
}

pub fn get_key_from_property_map(map: &PropertyMap) -> Result<Option<MapString>, HolonError> {
    let key_option = map.get(&PropertyName(MapString("key".to_string())));
    if let Some(inner_value) = key_option {
        let string_value: String = inner_value.try_into().map_err(|_| {
            HolonError::UnexpectedValueType(format!("{:?}", inner_value), "MapString".to_string())
        })?;
        Ok(Some(MapString(string_value)))
    } else {
        Ok(None)
    }
}

pub fn get_staged_holon_by_base_key(
    context: &dyn HolonsContextBehavior,
    key: &MapString,
) -> Result<StagedReference, HolonError> {
    let staging_service = context.get_space_manager().get_nursery_access();
    let staging_service = staging_service.read().map_err(|e| {
        HolonError::FailedToAcquireLock(format!("Failed to acquire read lock on nursery: {}", e))
    })?;
    staging_service.get_staged_holon_by_base_key(key)
}

pub fn get_staged_holon_by_versioned_key(
    context: &dyn HolonsContextBehavior,
    key: &MapString,
) -> Result<StagedReference, HolonError> {
    let staging_service = context.get_space_manager().get_nursery_access();
    let staging_service = staging_service.read().map_err(|e| {
        HolonError::FailedToAcquireLock(format!("Failed to acquire read lock on nursery: {}", e))
    })?;
    staging_service.get_staged_holon_by_versioned_key(key)
}

pub fn get_transient_holon_by_base_key(
    context: &dyn HolonsContextBehavior,
    key: &MapString,
) -> Result<TransientReference, HolonError> {
    let transient_service = context.get_space_manager().get_transient_behavior_service();
    let transient_service = transient_service.read().map_err(|e| {
        HolonError::FailedToAcquireLock(format!(
            "Failed to acquire read lock on transient_behavior_service: {}",
            e
        ))
    })?;
    transient_service.get_transient_holon_by_base_key(key)
}

pub fn get_transient_holon_by_versioned_key(
    context: &dyn HolonsContextBehavior,
    key: &MapString,
) -> Result<TransientReference, HolonError> {
    let transient_service = context.get_space_manager().get_transient_behavior_service();
    let transient_service = transient_service.read().map_err(|e| {
        HolonError::FailedToAcquireLock(format!(
            "Failed to acquire read lock on transient_behavior_service: {}",
            e
        ))
    })?;
    transient_service.get_transient_holon_by_versioned_key(key)
}

// == //

// ==== STAGING ====

/// Stages a new holon as a clone of the original holon.
///
/// This function creates a new holon in the staging area by cloning the `original_holon`,
/// while retaining a lineage relationship back to the `original_holon`. Use this function
/// when you want to create a new instance based on an existing holon while preserving its
/// ancestral link.
///
/// For staging a new version of an existing holon (i.e., where the original is a predecessor),
/// use [`stage_new_version`].
///
/// # Arguments
/// - `context`: The context to retrieve holon services.
/// - `original_holon`: A reference to the holon to be cloned.
///
/// # Returns
/// - `Ok(StagedReference)` pointing to the newly staged holon.
/// - `Err(HolonError)` if staging fails.
///
/// # Errors
/// - Returns a `HolonError` if the staging operation cannot complete.
///
pub fn stage_new_from_clone(
    context: &dyn HolonsContextBehavior,
    original_holon: HolonReference,
    new_key: MapString,
) -> Result<StagedReference, HolonError> {
    let staging_service = context.get_space_manager().get_holon_service();
    let staged_reference =
        staging_service.stage_new_from_clone_internal(context, original_holon, new_key)?;

    Ok(staged_reference)
}

/// Stages a new holon in the holon space.
///
/// This function creates a new holon in the staging area without any lineage
/// relationship to an existing holon. Use this function for creating entirely
/// new holons that are not tied to any predecessor.
///
/// # Arguments
/// - `context`: The context to retrieve holon services.
/// - `holon`: The new holon to stage.
///
/// # Returns
/// - `Ok(StagedReference)` pointing to the newly staged holon.
/// - `Err(HolonError)` if staging fails.
///
/// # Errors
/// - Returns a `HolonError` if the staging operation cannot complete.
///
pub fn stage_new_holon(
    context: &dyn HolonsContextBehavior,
    transient_reference: TransientReference,
) -> Result<StagedReference, HolonError> {
    let staging_service = context.get_space_manager().get_nursery_access();
    let staged_reference = staging_service
        .read()
        .map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on nursery: {}",
                e
            ))
        })?
        .stage_new_holon(context, transient_reference)?;

    Ok(staged_reference)
}

/// Stages a new holon as a version of the current holon.
///
/// This function creates a new holon in the staging area by cloning the `current_version`
/// and marking it as its predecessor. Use this function when creating a **new version**
/// of an existing holon with a clear lineage relationship.
///
/// For creating a clone without retaining a lineage relationship, use [`stage_new_from_clone`].
///
/// # Arguments
/// - `context`: The context to retrieve holon services.
/// - `current_version`: A smart reference to the current version of the holon.
///
/// # Returns
/// - `Ok(StagedReference)` pointing to the newly staged holon.
/// - `Err(HolonError)` if staging fails.
///
/// # Errors
/// - Returns a `HolonError` if the staging operation cannot complete.
///
pub fn stage_new_version(
    context: &dyn HolonsContextBehavior,
    current_version: SmartReference,
) -> Result<StagedReference, HolonError> {
    let holon_service = context.get_space_manager().get_holon_service();
    let staged_reference = holon_service.stage_new_version_internal(context, current_version)?;

    Ok(staged_reference)
}

// ======

// Standalone function to summarize a vector of Holons
pub fn summarize_holons(holons: &Vec<Holon>) -> String {
    let summaries: Vec<String> = holons.iter().map(|holon| holon.summarize()).collect();
    format!("Holons: [{}]", summaries.join(", "))
}

// Gets total count of Staged Holons present in the Nursery
pub fn staged_count(context: &dyn HolonsContextBehavior) -> Result<i64, HolonError> {
     return context
        .get_space_manager()
        .get_nursery_access()
        .read()
        .map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire read lock on nursery: {}", e))
        })?
        .staged_count();
}

// Gets total count of Transient Holons present in the TransientHolonManager
pub fn transient_count(context: &dyn HolonsContextBehavior) -> Result<i64, HolonError> {
    return context
        .get_space_manager()
        .get_transient_behavior_service()
        .read()
        .expect("Failed to acquire read lock on transient_behavior_service")
        .transient_count()
}
