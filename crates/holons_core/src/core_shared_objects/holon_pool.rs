use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use super::{Holon, HolonBehavior};
use crate::utils::uuid::create_temporary_id_from_key;
use base_types::MapString;
use core_types::{HolonError, TemporaryId};

//
// === HolonPool NewTypes ===
//

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TransientHolonPool(pub HolonPool);

impl From<HolonPool> for TransientHolonPool {
    fn from(pool: HolonPool) -> Self {
        TransientHolonPool(pool)
    }
}

impl Deref for TransientHolonPool {
    type Target = HolonPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TransientHolonPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StagedHolonPool(pub HolonPool);

impl From<HolonPool> for StagedHolonPool {
    fn from(pool: HolonPool) -> Self {
        StagedHolonPool(pool)
    }
}

impl Deref for StagedHolonPool {
    type Target = HolonPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StagedHolonPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TransientSerializableHolonPool(pub SerializableHolonPool);

impl Deref for TransientSerializableHolonPool {
    type Target = SerializableHolonPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TransientSerializableHolonPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StagedSerializableHolonPool(pub SerializableHolonPool);

impl Deref for StagedSerializableHolonPool {
    type Target = SerializableHolonPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StagedSerializableHolonPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

//
// === SerializableHolonPool ===
//

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SerializableHolonPool {
    pub holons: BTreeMap<TemporaryId, Holon>,
    pub keyed_index: BTreeMap<MapString, TemporaryId>,
}

impl Default for SerializableHolonPool {
    fn default() -> Self {
        Self { holons: BTreeMap::new(), keyed_index: BTreeMap::new() }
    }
}

//
// === HolonPool ===
//

/// A general-purpose container that manages owned Holons with key-based and index-based lookups.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HolonPool {
    holons: BTreeMap<TemporaryId, Rc<RefCell<Holon>>>,
    keyed_index: BTreeMap<MapString, TemporaryId>,
}

impl From<SerializableHolonPool> for HolonPool {
    fn from(pool: SerializableHolonPool) -> Self {
        let mut holons = BTreeMap::new();
        for (id, holon) in pool.holons {
            holons.insert(id, Rc::new(RefCell::new(holon)));
        }
        Self { holons, keyed_index: pool.keyed_index.clone() }
    }
}

impl HolonPool {
    /// Creates an empty HolonPool
    pub fn new() -> Self {
        Self { holons: BTreeMap::new(), keyed_index: BTreeMap::new() }
    }

    /// Clears all Holons and their associated key mappings.
    pub fn clear(&mut self) {
        self.holons.clear();
        self.keyed_index.clear();
    }

    /// Get a vector of references to the Holons in the HolonPool.
    ///
    /// ⚠️ Only intended during the commit process due to mutable access risks.
    pub fn get_all_holons(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.holons.values().cloned().collect()
    }

    /// Retrieves a Holon by its temporary id.
    pub fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.holons
            .get(id)
            .cloned()
            .ok_or_else(|| HolonError::HolonNotFound(format!("for id: {:?}", id)))
    }

    /// Retrieves a Holon by its versioned (unique) key.
    pub fn get_holon_by_versioned_key(&self, key: &MapString) -> Option<Rc<RefCell<Holon>>> {
        self.keyed_index.get(key).and_then(|id| self.holons.get(id).cloned())
    }

    /// Retrieves the temporary id of a Holon by its base key. Called when it is expected that there is only
    /// one Holon with an associated base key.
    pub fn get_id_by_base_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        let ids: Vec<&TemporaryId> = self
            .keyed_index
            .range(MapString(key.0.clone())..)
            .take_while(|(k, _)| k.0.starts_with(&key.0))
            .map(|(_, v)| v)
            .collect();

        if ids.is_empty() {
            return Err(HolonError::HolonNotFound(format!("for key: {}", key)));
        }
        if ids.len() > 1 {
            return Err(HolonError::DuplicateError("Holons".to_string(), format!("key: {}", key)));
        }

        Ok(ids[0].clone())
    }

    /// Returns TemporaryIds for all Holons with the same base key. Called when there may be multiple Holons for
    /// a base key.
    pub fn get_ids_by_base_key(&self, key: &MapString) -> Result<Vec<&TemporaryId>, HolonError> {
        let ids: Vec<&TemporaryId> = self
            .keyed_index
            .range(MapString(key.0.clone())..)
            .take_while(|(k, _)| k.0.starts_with(&key.0))
            .map(|(_, v)| v)
            .collect();

        if ids.is_empty() {
            return Err(HolonError::HolonNotFound(format!("for key: {}", key)));
        }

        Ok(ids)
    }

    /// Retrieves the temporary id of a Holon by its versioned key.
    pub fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        self.keyed_index
            .get(key)
            .cloned()
            .ok_or_else(|| HolonError::HolonNotFound(format!("for key: {}", key)))
    }

    /// Exports the HolonPool as a `SerializableHolonPool`.
    pub fn export_pool(&self) -> SerializableHolonPool {
        let mut holons = BTreeMap::new();
        for (id, holon) in self.holons.iter() {
            holons.insert(id.clone(), holon.borrow().clone());
        }
        SerializableHolonPool { holons, keyed_index: self.keyed_index.clone() }
    }

    /// Imports a `SerializableHolonPool`, replacing the current holons.
    pub fn import_pool(&mut self, pool: SerializableHolonPool) {
        self.holons.clear();
        self.keyed_index.clear();

        for (id, holon) in pool.holons.into_iter() {
            self.holons.insert(id, Rc::new(RefCell::new(holon)));
        }

        self.keyed_index.extend(pool.keyed_index);
    }

    /// Inserts a new Holon into the pool.
    pub fn insert_holon(&mut self, mut holon: Holon) -> Result<TemporaryId, HolonError> {
        // Assuming that Holon Pools are only used in the Nursery or TransientManager
        // Discussion topic: maybe this could change in the future.. what would be the use case?
        if holon.is_saved() {
            return Err(HolonError::InvalidType(
                "Saved Holons are not allowed to be added to Holon Pools (at least for now)"
                    .to_string(),
            ));
        } else {
            let mut versioned_key = holon.get_versioned_key()?;

            while self.keyed_index.get(&versioned_key).is_some() {
                holon.increment_version()?;
                versioned_key = holon.get_versioned_key()?;
            }
            if holon.is_staged() {
                versioned_key.0 += "staged"
            }
            if holon.is_transient() {
                versioned_key.0 += "transient"
            }

            let id = create_temporary_id_from_key(&versioned_key);

            self.keyed_index.insert(versioned_key, id.clone());
            self.holons.insert(id.clone(), Rc::new(RefCell::new(holon)));

            Ok(id)
        }
    }

    /// Returns the number of Holons in the pool.
    pub fn len(&self) -> usize {
        self.holons.len()
    }
}
