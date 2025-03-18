use crate::core_shared_objects::{Holon, HolonError};
use crate::utils::uuid::{generate_temporary_id, TemporaryId};
use hdi::prelude::{Deserialize, Serialize};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use uuid::Uuid;

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
    /// Returns a reference to the internal vector of staged Holons.
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
    pub fn get_all_holons(&self) -> impl Iterator<Item = Rc<RefCell<Holon>>> + '_ {
        self.holons.values().cloned()
    }

    /// Retrieves a Holon by its key.
    pub fn get_holon_by_key(&self, key: &MapString) -> Option<Rc<RefCell<Holon>>> {
        self.keyed_index.get(key).and_then(|id| self.holons.get(id).cloned())
    }

    /// Retrieves a Holon by its temporary id.
    pub fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.holons
            .get(id)
            .cloned()
            .ok_or_else(|| HolonError::HolonNotFound(format!("for id: {:?}", id)))
    }

    /// Retrieves the temporary id of a Holon by its key.
    pub fn get_id_by_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
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
        self.keyed_index.clear(); // Remove existing index

        // Populate with new holons
        for (id, holon) in pool.holons.into_iter() {
            self.holons.insert(id, Rc::new(RefCell::new(holon)));
        }

        self.keyed_index.extend(pool.keyed_index);
    }

    /// Inserts a new Holon into the pool and if it has a key, update the keyed_index. Returns its TemporaryId.
    /// NOTE: Silently ignores a potential is_accessible error from get_key because it assumes acccessiblity is checked by the caller.
    ///
    /// # Arguments
    /// - `holon` - The Holon to be inserted.
    ///
    /// # Returns
    /// - `TemporaryId` representing the index where the Holon was inserted.
    pub fn insert_holon(&mut self, holon: Holon) -> TemporaryId {
        // Create random id.
        let id = generate_temporary_id();

        // Update index if Holon has a key.
        if let Ok(Some(key)) = &holon.get_key() {
            self.keyed_index.insert(key.clone(), id.clone());
        }

        // update pool
        let rc_holon = Rc::new(RefCell::new(holon));
        self.holons.insert(id.clone(), rc_holon);

        id
    }
    /// Returns the number of Holons in the pool.
    ///
    /// # Returns
    /// - `usize` representing the total count of Holons in the pool.
    pub fn len(&self) -> usize {
        self.holons.len()
    }
}
