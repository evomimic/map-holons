use std::{any::Any, sync::{Arc, RwLock}};

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, PropertyMap, PropertyName, TemporaryId};
use tracing::{debug, warn};

use crate::{
    core_shared_objects::{
        holon::{
            state::{HolonState, ValidationState},
            Holon, HolonCloneModel, TransientHolon,
        },
        holon_pool::{SerializableHolonPool, TransientHolonPool},
        transient_manager_access_internal::TransientManagerAccessInternal,
        ReadableRelationship, TransientManagerAccess, TransientRelationshipMap,
    },
    reference_layer::{TransientHolonBehavior, TransientReference},
    HolonPool, HolonsContextBehavior,
};

/// Holon variant-agnostic interface for cloning.
///
/// Regardless of the source phase, cloned Holons always begin their lifecycle as `TransientHolon`.
pub trait ToHolonCloneModel {
    fn holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError>;
}

#[derive(Debug)]
pub struct TransientHolonManager {
    transient_holons: RwLock<TransientHolonPool>, // Thread-safe pool
}

impl TransientHolonManager {
    pub fn new_empty() -> Self {
        Self { transient_holons: RwLock::new(TransientHolonPool(HolonPool::new())) }
    }

    pub fn new_with_pool(pool: TransientHolonPool) -> Self {
        Self { transient_holons: RwLock::new(pool) }
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

    /// This function converts a TemporaryId into a TransientReference.
    /// Returns HolonError::HolonNotFound if id is not present in the holon pool.
    fn to_validated_transient_reference(
        &self,
        id: &TemporaryId,
    ) -> Result<TransientReference, HolonError> {
        // Determine if the id references a TransientHolon in the transient manager
        let _ = self.get_holon_by_id(id)?;
        Ok(TransientReference::from_temporary_id(id))
    }
}

impl Clone for TransientHolonManager {
    // Clone underlying pool for thread-safe manager
        fn clone(&self) -> Self {
        let pool = self
            .transient_holons
            .read()
            .expect("Failed to acquire read lock on transient_holons")
            .clone();
        TransientHolonManager::new_with_pool(pool)
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
        property_map
            .insert(PropertyName(MapString("key".to_string())), BaseValue::StringValue(key));
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
            self.transient_holons.try_read().map_err(|e| {
                warn!("❌ Failed to acquire read lock");
                HolonError::FailedToAcquireLock(format!("Read lock unavailable: {}", e))
            })?.get_id_by_base_key(key)?
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
            self.transient_holons.try_read().map_err(|e| {
                warn!("❌ Failed to acquire read lock");
                HolonError::FailedToAcquireLock(format!("Read lock unavailable: {}", e))
            })?.get_id_by_versioned_key(key)?
        };
        self.to_validated_transient_reference(&id)
    }

    fn transient_count(&self) -> Result<i64, HolonError> {
        let pool = self.transient_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire read lock on transient_holons: {}", e))
        })?;
        Ok(pool.len() as i64)
    }
}

impl TransientManagerAccessInternal for TransientHolonManager {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clear_pool(&mut self) {
        let mut pool = self.transient_holons.write().unwrap();
        pool.clear();
    }

    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        self.transient_holons.read().unwrap().get_id_by_versioned_key(key)
    }

    fn get_transient_holons_pool(&self) -> Result<Vec<Arc<RwLock<Holon>>>, HolonError>  {
        let pool = match self.transient_holons.try_read() {
            Ok(guard) => guard,
            Err(e) => {
                return Err(HolonError::FailedToAcquireLock(format!("Failed to acquire read lock for get_transient_holons_pool: {}", e)));
            }
        };
        // Return shared Arc pointers for thread-safe access
        Ok(
            pool
            .get_all_holons()
            .into_iter()
            .map(|rc_ref| rc_ref.clone())
            .collect::<Vec<_>>()
        )
    }

    fn export_transient_holons(&self) -> Result<SerializableHolonPool, HolonError>  {
        self.transient_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire read lock for export_transient_holons: {}", e))
        })?.export_pool()
    }

    fn import_transient_holons(&mut self, pool: SerializableHolonPool) -> () {
        self.transient_holons.try_write().unwrap().import_pool(pool); // Mutates existing HolonPool
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