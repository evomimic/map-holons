use super::holon::{Holon, HolonBehavior};
use crate::utils::uuid::create_temporary_id_from_key;
use crate::HolonError;
use base_types::MapString;
use core_types::TemporaryId;
use hdi::prelude::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A general-purpose container that manages owned Holons with key-based and index-based lookups.

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HolonPool {
    holons: BTreeMap<TemporaryId, Rc<RefCell<Holon>>>, // Stores Holons with shared ownership
    keyed_index: BTreeMap<MapString, TemporaryId>,     // Maps keys to Holon indices
}

/// Struct for exporting and importing HolonPool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SerializableHolonPool {
    pub holons: BTreeMap<TemporaryId, Holon>, // Directly stores Holon values in serializable form
    pub keyed_index: BTreeMap<MapString, TemporaryId>, // Keyed index remains unchanged
}
impl Default for SerializableHolonPool {
    fn default() -> Self {
        Self { holons: BTreeMap::new(), keyed_index: BTreeMap::new() }
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
    /// # ⚠️ Caution:
    /// - This method is **intended solely for use during the commit process**.
    /// - It provides **direct access** to the `Vec<Rc<RefCell<Holon>>>`, meaning:
    ///   - The caller can **modify the vector itself** (e.g., add/remove Holons).
    ///   - The caller can **mutate individual Holons** via `RefCell` borrowing.
    /// - **This access is necessary** because commit processing updates each Holon's
    ///   state and error tracking.
    ///
    /// # Future Considerations:
    /// While a read-only iterator is **not an option** due to required Holon mutations,
    /// future refactors could explore safer ways to expose staged Holons.
    ///
    /// # Returns
    /// An iterator of Rc<RefCell< staged Holons.
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

    /// Retrieves the temporary id of a Holon by its base key.
    /// Convenience method for retrieving a single StagedReference for a base key, when the caller expects there to only be one.
    /// Returns a duplicate error if multiple found.
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

    /// Returns TemporaryId's for all Holons that have the same base key.
    /// This can be useful if multiple versions of the same Holon are being staged at the same time.
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

    /// Retrieves the temporary id of a Holon by its (unique) versioned key.
    pub fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        self.keyed_index
            .get(key)
            .cloned()
            .ok_or_else(|| HolonError::HolonNotFound(format!("for key: {}", key)))
    }

    /// Exports the staged holons as a `SerializableHolonPool`.
    ///
    /// This method **clones** the HolonPool data into a structure suitable for serialization,
    /// ensuring that the exported state is independent of the internal HolonPool.
    ///
    /// # Returns
    /// A `SerializableHolonPool` containing a deep clone of the staged holons and keyed index.
    pub fn export_pool(&self) -> SerializableHolonPool {
        let mut holons = BTreeMap::new();
        // Convert Rc<RefCell<Holon>> → Holon
        for (id, holon) in self.holons.iter() {
            holons.insert(id.clone(), holon.borrow().clone());
        }
        SerializableHolonPool { holons, keyed_index: self.keyed_index.clone() }
    }

    /// Imports a `SerializableHolonPool`, replacing the current staged holons.
    ///
    /// This method **replaces** the current HolonPool state with the imported holons and indexed keys.
    ///
    /// # Arguments
    /// - `pool` - A `SerializableHolonPool` containing the staged holons and their keyed index.
    pub fn import_pool(&mut self, pool: SerializableHolonPool) -> () {
        self.holons.clear(); // Remove existing holons
        self.keyed_index.clear(); // Remove existing index

        // Populate with new holons
        for (id, holon) in pool.holons.into_iter() {
            self.holons.insert(id, Rc::new(RefCell::new(holon)));
        }

        self.keyed_index.extend(pool.keyed_index);
    }

    ////
    // ==== TEMPORARY WORKAROUND === //
    //  -- Until client is functional and we can call a generate random number dance --
    ////
    ///
    /// Inserts a new Holon into the pool and updates the keyed_index with its versioned_key. Returns its TemporaryId (first 16 bytes of sha2 hash of its 'key').
    ///
    /// NOTE: Silently ignores a potential is_accessible error from get_key because it assumes acccessiblity is checked by the caller.
    ///
    /// # Arguments
    /// - `holon` - The Holon to be inserted.
    ///
    /// # Returns
    /// - `TemporaryId` representing the index where the Holon was inserted.
    pub fn insert_holon(&mut self, mut holon: Holon) -> Result<TemporaryId, HolonError> {
        // Concatenate base_key with version_sequence_count
        let mut versioned_key = holon.get_versioned_key()?;

        // Check for existing, if found, increment count
        while self.keyed_index.get(&versioned_key).is_some() {
            holon.increment_version()?;
            versioned_key = holon.get_versioned_key()?;
        }

        // Create temporary id
        let id = create_temporary_id_from_key(&versioned_key);

        // Update index
        self.keyed_index.insert(versioned_key, id.clone());

        // Update pool
        let rc_holon = Rc::new(RefCell::new(holon));

        self.holons.insert(id.clone(), rc_holon);

        Ok(id)
    }

    /// Returns the number of Holons in the pool.
    ///
    /// # Returns
    /// - `usize` representing the total count of Holons in the pool.
    pub fn len(&self) -> usize {
        self.holons.len()
    }
}
