use super::{holon_pool::HolonPool, nursery_access_internal::NurseryAccessInternal, Holon};
use crate::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use crate::{
    core_shared_objects::StagedHolon,
    reference_layer::{HolonStagingBehavior, StagedReference, TransientReference},
    HolonReference, ReadableHolon, SmartReference, WritableHolon,
};
use crate::{
    core_shared_objects::{
        holon_pool::StagedHolonPool, transient_holon_manager::ToHolonCloneModel,
    },
    NurseryAccess,
};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, TemporaryId};
use std::{
    any::Any,
    sync::{Arc, RwLock, Weak},
};
use type_names::CorePropertyTypeName;

#[derive(Clone, Debug)]
pub struct Nursery {
    tx_id: TxId,
    context: Weak<TransactionContext>,
    staged_holons: Arc<RwLock<StagedHolonPool>>,
}

// The Nursery uses `Arc<RwLock<StagedHolonPool>>` to allow thread-safe mutation of staged holons.
// Each holon is stored in a `HolonPool`, where Holons are individually wrapped in `Arc<RwLock<Holon>>`.
// This structure allows:
//
// - Shared concurrent reads across threads (e.g. for base key lookups)
// - Exclusive mutation when staging or clearing
// - Safe export/import of holons via a runtime `HolonPool`
// - Isolation of each staged holon for commit preparation, minimizing lock contention.
//
// All read/write access to the staged pool is explicitly scoped to avoid lock poisoning or panic scenarios.
impl Nursery {
    // /// Creates a new Nursery with an empty HolonPool
    pub fn new(tx_id: TxId, context: Weak<TransactionContext>) -> Self {
        Self {
            tx_id,
            context,
            staged_holons: Arc::new(RwLock::new(StagedHolonPool(HolonPool::new()))),
        }
    }

    /// Stages a new holon.
    ///
    /// # Arguments
    /// * `holon` - A reference to the holon to be staged.
    ///
    /// # Returns
    /// The TemporaryId, which is used a unique identifier.
    fn stage_holon(&self, holon: StagedHolon) -> Result<TemporaryId, HolonError> {
        let mut pool = self.staged_holons.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged_holons: {}",
                e
            ))
        })?;
        let id = pool.insert_holon(Holon::Staged(holon))?;
        Ok(id)
    }

    /// This function converts a TemporaryId into a StagedReference.
    /// Returns HolonError::HolonNotFound if id is not present in the holon pool.
    fn to_validated_staged_reference(
        &self,
        id: &TemporaryId,
    ) -> Result<StagedReference, HolonError> {
        // Determine if the id references a StagedHolon in the Nursery
        let _holon_rc = self.get_holon_by_id(id)?;

        let transaction_handle = self.require_handle()?;
        Ok(StagedReference::from_temporary_id(transaction_handle.clone(), id))
    }

    fn require_handle(&self) -> Result<TransactionContextHandle, HolonError> {
        let context = self.context.upgrade().ok_or_else(|| {
            HolonError::ServiceNotAvailable(format!(
                "TransactionContext (tx_id={})",
                self.tx_id.value()
            ))
        })?;

        // Optional extra guard: assert context.tx_id() == self.tx_id
        // If this ever fails, it's a serious invariant break.
        if context.tx_id() != self.tx_id {
            return Err(HolonError::CrossTransactionReference {
                reference_kind: "Nursery".to_string(),
                reference_id: format!("TxId={}", self.tx_id.value()),
                reference_tx: self.tx_id.value(),
                context_tx: context.tx_id().value(),
            });
        }

        Ok(TransactionContextHandle::new(context))
    }
}

impl NurseryAccess for Nursery {
    /// Retrieves a staged holon by index.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Arc<RwLock<Holon>>, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        pool.get_holon_by_id(id)
    }
}

impl HolonStagingBehavior for Nursery {
    // Caller is assuming there is only one, returns duplicate error if multiple.
    fn get_staged_holon_by_base_key(&self, key: &MapString) -> Result<StagedReference, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        let id = pool.get_id_by_base_key(key)?;
        self.to_validated_staged_reference(&id)
    }

    fn get_staged_holons_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<Vec<StagedReference>, HolonError> {
        let mut staged_references = Vec::new();
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        let ids = pool.get_ids_by_base_key(key)?;
        for id in ids {
            let validated_staged_reference = self.to_validated_staged_reference(&id)?;
            staged_references.push(validated_staged_reference);
        }

        Ok(staged_references)
    }

    /// Does a lookup by full (unique) key on staged holons.
    fn get_staged_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<StagedReference, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        let id = pool.get_id_by_versioned_key(key)?;
        self.to_validated_staged_reference(&id)
    }

    fn staged_count(&self) -> Result<i64, HolonError> {
        let count = self
            .staged_holons
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged_holons: {}",
                    e
                ))
            })?
            .len() as i64;
        Ok(count)
    }

    fn stage_new_holon(
        &self,
        transient_reference: TransientReference,
    ) -> Result<StagedReference, HolonError> {
        let staged_holon =
            StagedHolon::new_from_clone_model(transient_reference.holon_clone_model()?)?;
        let new_id = self.stage_holon(staged_holon)?;
        self.to_validated_staged_reference(&new_id)
    }

    /// Stage a new holon by cloning an existing holon, with a new key and
    /// *without* maintaining lineage to the original.
    fn stage_new_from_clone(
        &self,
        original_holon: HolonReference,
        new_key: MapString,
    ) -> Result<StagedReference, HolonError> {
        // Clone into a transient holon
        let mut cloned_transient = original_holon.clone_holon()?;

        // Overwrite the Key property on the clone
        let key_prop = CorePropertyTypeName::Key.as_property_name();
        cloned_transient.with_property_value(key_prop, BaseValue::StringValue(new_key))?;

        // Reset original_id (this is a new clone, not a new version)
        cloned_transient.reset_original_id()?;

        // Stage the cloned holon
        let mut cloned_staged = self.stage_new_holon(cloned_transient)?;

        // Explicitly clear predecessor for "from clone" semantics
        cloned_staged.with_predecessor(None)?;

        Ok(cloned_staged)
    }

    /// Stage a new holon as a *version* of the current holon, keeping lineage.
    fn stage_new_version(
        &self,
        current_version: SmartReference,
    ) -> Result<StagedReference, HolonError> {
        // Clone current version into a transient holon
        let cloned_transient = current_version.clone_holon()?;

        // Stage it as a new holon
        let mut cloned_staged = self.stage_new_holon(cloned_transient)?;

        // Set predecessor back to the current version
        cloned_staged.with_predecessor(Some(HolonReference::Smart(current_version)))?;

        Ok(cloned_staged)
    }
}

impl NurseryAccessInternal for Nursery {
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// Clears all staged holons.
    fn clear_stage(&self) -> Result<(), HolonError> {
        // Failure to acquire the lock is propagated as an error rather than panicking.
        let mut guard = self.staged_holons.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged_holons: {}",
                e
            ))
        })?;
        guard.clear();
        Ok(())
    }

    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        pool.get_id_by_versioned_key(key)
    }

    /// Exports the staged holons as a runtime `HolonPool`.
    fn export_staged_holons(&self) -> Result<HolonPool, HolonError> {
        let staged_pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        Ok(staged_pool.0.clone())
    }

    /// Replaces the current staged holons with those from the provided `HolonPool`.
    fn import_staged_holons(&self, pool: HolonPool) -> Result<(), HolonError> {
        let mut guard = self.staged_holons.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged_holons: {}",
                e
            ))
        })?;
        guard.import_pool(pool); // Mutates the existing pool
        Ok(())
    }

    /// Returns a reference-layer view of all staged holons as `StagedReference`s.
    ///
    /// This is the main entry for the commit pipeline and avoids exposing
    /// the underlying HolonPool or `Arc<RwLock<Holon>>` handles.
    fn get_staged_references(&self) -> Result<Vec<StagedReference>, HolonError> {
        let transaction_handle = self.require_handle()?;
        let guard = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        Ok(guard.get_staged_references(transaction_handle))
    }
}
