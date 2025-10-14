use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use super::{
    holon_pool::{HolonPool, SerializableHolonPool},
    nursery_access_internal::NurseryAccessInternal,
    Holon,
};
use crate::{
    core_shared_objects::StagedHolon,
    reference_layer::{HolonStagingBehavior, StagedReference, TransientReference},
    HolonsContextBehavior,
};
use crate::{
    core_shared_objects::{
        holon_pool::StagedHolonPool, transient_holon_manager::ToHolonCloneModel,
    },
    NurseryAccess,
};
use base_types::MapString;
use core_types::{HolonError, TemporaryId};

#[derive(Clone, Debug)]
pub struct Nursery {
    staged_holons: Arc<RwLock<StagedHolonPool>>, // Thread-safe pool of staged holons
}

// The Nursery uses `Arc<RwLock<StagedHolonPool>>` to allow thread-safe mutation of staged holons.
// Each holon is stored in a `HolonPool`, where Holons are individually wrapped in `Arc<RwLock<Holon>>`.
// This structure allows:
//
// - Shared concurrent reads across threads (e.g. for base key lookups)
// - Exclusive mutation when staging or clearing
// - Safe export/import of holons via `SerializableHolonPool`
// - Isolation of each staged holon for commit preparation, minimizing lock contention.
//
// All read/write access to the staged pool is explicitly scoped to avoid lock poisoning or panic scenarios.
impl Nursery {
    /// Creates a new Nursery with an empty HolonPool
    pub fn new() -> Self {
        Self { staged_holons: Arc::new(RwLock::new(StagedHolonPool(HolonPool::new()))) }
    }

    // pub fn as_internal(&self) -> &dyn NurseryAccessInternal {
    //     self
    // }

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

        Ok(StagedReference::from_temporary_id(id))
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

    fn staged_count(&self) -> i64 {
        self.staged_holons.read().expect("Failed to acquire read lock on staged_holons").len()
            as i64
    }

    fn stage_new_holon(
        &self,
        context: &dyn HolonsContextBehavior,
        transient_reference: TransientReference,
    ) -> Result<StagedReference, HolonError> {
        let staged_holon =
            StagedHolon::new_from_clone_model(transient_reference.get_holon_clone_model(context)?)?;
        let new_id = self.stage_holon(staged_holon)?;
        self.to_validated_staged_reference(&new_id)
    }
}

impl NurseryAccessInternal for Nursery {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clear_stage(&mut self) {
        self.staged_holons.write().expect("Failed to acquire write lock on staged_holons").clear();
    }

    // fn get_keyed_index(&self) -> BTreeMap<MapString, usize> {
    //     self.holon_store.borrow().keyed_index.clone()
    // }

    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        pool.get_id_by_versioned_key(key)
    }

    /// Exports the staged holons using `SerializableHolonPool`
    fn export_staged_holons(&self) -> SerializableHolonPool {
        self.staged_holons
            .read()
            .expect("Failed to acquire read lock on staged_holons")
            .export_pool()
    }

    fn import_staged_holons(&mut self, pool: SerializableHolonPool) -> () {
        self.staged_holons
            .write()
            .expect("Failed to acquire write lock on staged_holons")
            .import_pool(pool); // Mutates existing HolonPool
    }

    /// Returns the staged Holons in the `HolonPool`,
    /// ensuring that commit functions can access the actual Holon instances.
    // fn get_holons_to_commit(&self) -> impl Iterator<Item = Rc<RefCell<Holon>>> + '_ {
    /// Retrieves the staged Holon instances for commit, using thread-safe handles
    fn get_holons_to_commit(&self) -> Vec<Arc<RwLock<Holon>>> {
        self.staged_holons
            .read()
            .expect("Failed to acquire read lock on staged_holons")
            .get_all_holons()
    }

    // fn stage_holon(&self, holon: Holon) -> usize {
    //     self.staged_holons.borrow_mut().insert_holon(holon)
    // }
}
