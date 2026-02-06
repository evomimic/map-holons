use std::{
    any::Any,
    sync::{Arc, RwLock, Weak},
};

use crate::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use crate::{
    core_shared_objects::{
        holon::{
            state::{HolonState, ValidationState},
            Holon, HolonCloneModel, TransientHolon,
        },
        holon_pool::{HolonPool, TransientHolonPool},
        transient_manager_access_internal::TransientManagerAccessInternal,
        ReadableRelationship, TransientManagerAccess, TransientRelationshipMap,
    },
    reference_layer::{TransientHolonBehavior, TransientReference},
};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, PropertyMap, TemporaryId};
use tracing::warn;
use type_names::CorePropertyTypeName;

/// Holon variant-agnostic interface for cloning.
///
/// Regardless of the source phase, cloned Holons always begin their lifecycle as `TransientHolon`.
pub trait ToHolonCloneModel {
    fn holon_clone_model(&self) -> Result<HolonCloneModel, HolonError>;
}

#[derive(Debug)]
pub struct TransientHolonManager {
    tx_id: TxId,
    context: Weak<TransactionContext>,
    transient_holons: RwLock<TransientHolonPool>,
}

impl TransientHolonManager {
    pub fn new_empty(tx_id: TxId, context: Weak<TransactionContext>) -> Self {
        Self { tx_id, context, transient_holons: RwLock::new(TransientHolonPool(HolonPool::new())) }
    }

    pub fn new_with_pool(
        tx_id: TxId,
        context: Weak<TransactionContext>,
        pool: TransientHolonPool,
    ) -> Self {
        Self { tx_id, context, transient_holons: RwLock::new(pool) }
    }

    /// Adds the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the transient holon
    /// to be retrieved by key.
    fn add_new_holon(&self, holon: TransientHolon) -> Result<TransientReference, HolonError> {
        let id: TemporaryId = {
            let mut pool = self.transient_holons.try_write().map_err(|e| {
                warn!("❌ Failed to acquire write lock - lock may be held elsewhere");
                HolonError::FailedToAcquireLock(format!("Write lock unavailable: {}", e))
            })?;
            pool.insert_holon(Holon::Transient(holon))?
        };
        self.to_validated_transient_reference(&id)
    }

    /// Converts a TemporaryId into a validated TransientReference using the provided transaction handle.
    ///
    /// This enforces that:
    /// - the id exists in this manager’s pool
    /// - the reference is minted with the caller’s transaction handle
    pub(crate) fn to_validated_transient_reference(
        &self,
        id: &TemporaryId,
    ) -> Result<TransientReference, HolonError> {
        // Validate id exists in this pool.
        self.get_holon_by_id(id)?;

        // Mint the capability-bearing reference using a handle derived from the stored Weak<TC>.
        let handle = self.require_handle()?;

        Ok(TransientReference::from_temporary_id(handle, id))
    }

    fn require_handle(&self) -> Result<TransactionContextHandle, HolonError> {
        let context = self.context.upgrade().ok_or_else(|| {
            HolonError::ServiceNotAvailable(format!(
                "TransactionContext (tx_id={})",
                self.tx_id.value()
            ))
        })?;

        Ok(TransactionContextHandle::new(context))
    }
}

// Do we need to implement Clone for TransientHolonManager?
// Since the HolonSpaceManager holds an Arc<TransientHolonManager>, cloning the Arc
// can be done directly without needing to clone the manager itself.
// Something like `let tm = Arc::clone(&self.transient_manager)`
impl Clone for TransientHolonManager {
    fn clone(&self) -> Self {
        // NOTE: This clone is only used in limited internal contexts.
        // If the lock is poisoned here, we treat it as unrecoverable and panic,
        // rather than silently returning an incorrect "clone".
        let pool = self
            .transient_holons
            .read()
            .expect("Failed to acquire read lock on transient_holons while cloning manager")
            .clone();

        TransientHolonManager::new_with_pool(self.tx_id, self.context.clone(), pool)
    }
}

impl TransientManagerAccess for TransientHolonManager {
    /// Retrieves a transient holon by index.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Arc<RwLock<Holon>>, HolonError> {
        let pool = self.transient_holons.try_read().map_err(|e| {
            warn!("❌ Failed to acquire read lock");
            HolonError::FailedToAcquireLock(format!("Read lock unavailable: {}", e))
        })?;
        // Return the existing Arc<RwLock<Holon>> for shared ownership
        let arc_ref = pool.get_holon_by_id(id)?;
        Ok(Arc::clone(&arc_ref))
    }
}

impl TransientHolonBehavior for TransientHolonManager {
    fn create_empty(&self, key: MapString) -> Result<TransientReference, HolonError> {
        let mut property_map = PropertyMap::new();
        let key_property_name = CorePropertyTypeName::Key.as_property_name();
        property_map.insert(key_property_name, BaseValue::StringValue(key));
        let holon = TransientHolon::with_fields(
            MapInteger(1),
            HolonState::Mutable,
            ValidationState::ValidationRequired,
            // None,
            property_map,
            TransientRelationshipMap::new_empty(),
            None,
        );

        let transient_reference = self.add_new_holon(holon)?;

        Ok(transient_reference)
    }

    fn create_empty_without_key(&self) -> Result<TransientReference, HolonError> {
        let holon = TransientHolon::with_fields(
            MapInteger(1),
            HolonState::Mutable,
            ValidationState::ValidationRequired,
            PropertyMap::new(),
            TransientRelationshipMap::new_empty(),
            None,
        );
        self.add_new_holon(holon)
    }

    fn new_from_clone_model(
        &self,
        holon_clone_model: HolonCloneModel,
    ) -> Result<TransientReference, HolonError> {
        let transient_relationships = {
            if let Some(relationship_map) = holon_clone_model.relationships {
                relationship_map.clone_for_new_source()?
            } else {
                TransientRelationshipMap::new_empty()
            }
        };
        let holon = TransientHolon::with_fields(
            holon_clone_model.version,
            HolonState::Mutable,
            ValidationState::ValidationRequired,
            // None,
            holon_clone_model.properties,
            transient_relationships,
            holon_clone_model.original_id,
        );

        let transient_reference = self.add_new_holon(holon)?;

        Ok(transient_reference)
    }

    // Caller is assuming there is only one, returns duplicate error if multiple.
    fn get_transient_holon_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError> {
        let id = {
            self.transient_holons
                .try_read()
                .map_err(|e| {
                    warn!("❌ Failed to acquire read lock");
                    HolonError::FailedToAcquireLock(format!("Read lock unavailable: {}", e))
                })?
                .get_id_by_base_key(key)?
        };
        self.to_validated_transient_reference(&id)
    }

    fn get_transient_holons_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<Vec<TransientReference>, HolonError> {
        let mut transient_references = Vec::new();
        let transient_manager = self.transient_holons.try_read().map_err(|e| {
            warn!("❌ Failed to acquire read lock");
            HolonError::FailedToAcquireLock(format!("Read lock unavailable: {}", e))
        })?;
        let ids = transient_manager.get_ids_by_base_key(key)?;
        for id in ids {
            let validated_transient_reference = self.to_validated_transient_reference(&id)?;
            transient_references.push(validated_transient_reference);
        }

        Ok(transient_references)
    }

    fn get_transient_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError> {
        let id = {
            self.transient_holons
                .try_read()
                .map_err(|e| {
                    warn!("❌ Failed to acquire read lock");
                    HolonError::FailedToAcquireLock(format!("Read lock unavailable: {}", e))
                })?
                .get_id_by_versioned_key(key)?
        };
        self.to_validated_transient_reference(&id)
    }

    fn transient_count(&self) -> Result<i64, HolonError> {
        let pool = self.transient_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on transient_holons: {}",
                e
            ))
        })?;
        Ok(pool.len() as i64)
    }
}

impl TransientManagerAccessInternal for TransientHolonManager {
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// Clears all transient holons from the pool.
    fn clear_pool(&self) -> Result<(), HolonError> {
        // Lock failure returns a HolonError instead of panicking
        let mut pool = self.transient_holons.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock for clear_pool: {}",
                e
            ))
        })?;
        pool.clear();
        Ok(())
    }

    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        let pool = self.transient_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock for get_id_by_versioned_key: {}",
                e
            ))
        })?;
        pool.get_id_by_versioned_key(key)
    }

    fn get_transient_holons_pool(&self) -> Result<Vec<Arc<RwLock<Holon>>>, HolonError> {
        let pool = match self.transient_holons.try_read() {
            Ok(guard) => guard,
            Err(e) => {
                return Err(HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock for get_transient_holons_pool: {}",
                    e
                )));
            }
        };
        // Return shared Arc pointers for thread-safe access
        Ok(pool.get_all_holons().into_iter().map(|rc_ref| rc_ref.clone()).collect::<Vec<_>>())
    }

    fn export_transient_holons(&self) -> Result<HolonPool, HolonError> {
        Ok(self
            .transient_holons
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock for export_transient_holons: {}",
                    e
                ))
            })?
            .clone())
    }

    /// Imports holons into the transient pool, completely replacing existing holons.
    fn import_transient_holons(&self, pool: HolonPool) -> Result<(), HolonError> {
        let mut guard = self.transient_holons.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock for import_transient_holons: {}",
                e
            ))
        })?;
        guard.import_pool(pool); // Mutates the existing pool
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_thread_safe<T: Send + Sync>() {}

    #[test]
    fn transient_holon_manager_is_thread_safe() {
        assert_thread_safe::<TransientHolonManager>();
    }
}
