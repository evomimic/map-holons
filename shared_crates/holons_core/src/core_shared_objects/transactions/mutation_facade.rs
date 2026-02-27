use super::{TransactionContext, TransactionContextHandle};
use crate::{HolonReference, SmartReference, StagedReference, TransientReference};
use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId};
use std::sync::Arc;

/// Semantic facade for transaction-scoped mutation operations.
#[derive(Debug, Clone)]
pub struct MutationFacade {
    pub(crate) context: Arc<TransactionContext>,
}

impl MutationFacade {
    /// Creates a new TransientHolon.
    /// If `key` is `Some`, sets it at creation; if `None`, creates without a key.
    /// Returns a TransientReference to the newly created holon.
    pub fn new_holon(&self, key: Option<MapString>) -> Result<TransientReference, HolonError> {
        // Acquire transient service
        let borrowed_service = self.context.get_transient_behavior_service();

        let reference = match key {
            Some(key_string) => borrowed_service.create_empty(key_string)?,
            None => borrowed_service.create_empty_without_key()?,
        };

        Ok(reference)
    }

    /// Stages a new holon in the holon space.
    ///
    /// This function creates a new holon in the staging area, from a clone model of a TransientReference.
    /// Use this function as the 2nd step in the 'Holon Life Cycle'.
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
        &self,
        transient_reference: TransientReference,
    ) -> Result<StagedReference, HolonError> {
        self.context.ensure_open_for_mutation()?;
        let staged_reference =
            self.context.get_staging_service().stage_new_holon(transient_reference)?;

        Ok(staged_reference)
    }

    /// Stages a new holon as a clone of the original holon.
    ///
    /// This function creates a new holon (from either Staged or Smart) in the staging area by cloning the `original_holon`,
    /// without retaining a lineage relationship back to the original.
    ///
    /// For staging a new version of an existing holon (i.e., where the original is a
    /// predecessor), use [`stage_new_version`].
    ///
    /// # Arguments
    /// - `context`: The context to retrieve holon services.
    /// - `original_holon`: A Staged or Smart reference to the holon to be cloned.
    ///
    /// # Returns
    /// - `Ok(StagedReference)` pointing to the newly staged holon.
    /// - `Err(HolonError)` if staging fails.
    ///
    /// # Errors
    /// - If trying to pass a TransientReference, returns error directing to use 'stage_new_holon' instead.
    /// - Returns a `HolonError` if the staging operation cannot complete.
    ///
    pub fn stage_new_from_clone(
        &self,
        original_holon: HolonReference,
        new_key: MapString,
    ) -> Result<StagedReference, HolonError> {
        self.context.ensure_open_for_mutation()?;
        if original_holon.is_transient() {
            return Err(HolonError::InvalidHolonReference(
                "Must use stage_new_holon for staging from a TransientReference".to_string(),
            ));
        }
        self.context.get_staging_service().stage_new_from_clone(original_holon, new_key)
    }

    /// Stages a new holon as a version of the current holon.
    ///
    /// This function creates a new holon in the staging area by cloning the
    /// `current_version` and marking it as its predecessor. Use this function when
    /// creating a **new version** of an existing holon with a clear lineage
    /// relationship.
    ///
    /// For creating a clone without retaining a lineage relationship, use
    /// [`stage_new_from_clone`].
    ///
    /// # Arguments
    /// - `context`: The context to retrieve holon services.
    /// - `current_version`: A smart reference to the current version of the holon.
    ///
    /// # Returns
    /// - `Ok(StagedReference)` pointing to the newly staged holon.
    /// - `Err(HolonError)` if staging fails.
    pub fn stage_new_version(
        &self,
        current_version: SmartReference,
    ) -> Result<StagedReference, HolonError> {
        self.context.ensure_open_for_mutation()?;
        self.context.get_staging_service().stage_new_version(current_version)
    }

    pub fn stage_new_version_from_id(
        &self,
        holon_id: HolonId,
    ) -> Result<StagedReference, HolonError> {
        // Avoid constructing SmartReference at call sites (e.g. loader).
        // This keeps reference minting inside core execution surfaces/managers.

        // Build a tx-bound SmartReference using the context handle derived from the Arc.
        let handle = TransactionContextHandle::new(self.context.clone());
        let smart = SmartReference::new_from_id(handle, holon_id);

        // Lifecycle check happens inside stage_new_version
        self.stage_new_version(smart)
        // maybe later: staging_service.stage_new_version_from_id(holon_id)
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
    pub fn delete_holon(&self, local_id: LocalId) -> Result<(), HolonError> {
        self.context.ensure_open_for_mutation()?;
        self.context.get_holon_service().delete_holon_internal(&local_id)
    }

    pub fn load_holons(
        &self,
        bundle: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        self.context.ensure_open_for_mutation()?;
        self.context.get_holon_service().load_holons_internal(&self.context, bundle)
    }
}
