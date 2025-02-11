use crate::core_shared_objects::{Holon, HolonError};
use hdi::prelude::{Deserialize, Serialize};
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A general-purpose container that manages owned Holons with key-based and index-based lookups.

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HolonPool {
    holons: Vec<Rc<RefCell<Holon>>>, // Stores Holons with shared ownership
    keyed_index: BTreeMap<MapString, usize>, // Maps keys to Holon indices
}

/// Struct for exporting and importing HolonPool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SerializableHolonPool {
    pub holons: Vec<Holon>, // Directly stores Holon values in serializable form
    pub keyed_index: BTreeMap<MapString, usize>, // Keyed index remains unchanged
}
impl Default for SerializableHolonPool {
    fn default() -> Self {
        Self { holons: Vec::new(), keyed_index: BTreeMap::new() }
    }
}

impl HolonPool {
    /// Creates an empty HolonPool
    pub fn new() -> Self {
        Self { holons: Vec::new(), keyed_index: BTreeMap::new() }
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
    /// A reference to the vector of staged Holons.
    pub fn get_all_holons(&self) -> &Vec<Rc<RefCell<Holon>>> {
        &self.holons
    }

    /// Retrieves a Holon by its key.
    pub fn get_by_key(&self, key: &MapString) -> Option<Rc<RefCell<Holon>>> {
        self.keyed_index.get(key).and_then(|&index| self.holons.get(index).cloned())
    }

    /// Retrieves a Holon by its index.
    pub fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.holons
            .get(index)
            .cloned()
            .ok_or_else(|| HolonError::IndexOutOfRange(format!("No Holon at index {}", index)))
    }

    /// Retrieves the index of a Holon by its key.
    pub fn get_index_by_key(&self, key: &MapString) -> Result<usize, HolonError> {
        self.keyed_index
            .get(key)
            .cloned()
            .ok_or_else(|| HolonError::HolonNotFound(format!("No Holon found for key: {}", key)))
    }

    /// Exports the staged holons as a `SerializableHolonPool`.
    ///
    /// This method **clones** the HolonPool data into a structure suitable for serialization,
    /// ensuring that the exported state is independent of the internal HolonPool.
    ///
    /// # Returns
    /// A `SerializableHolonPool` containing a deep clone of the staged holons and keyed index.
    pub fn export_pool(&self) -> SerializableHolonPool {
        SerializableHolonPool {
            holons: self.holons.iter().map(|h| h.borrow().clone()).collect(), // Convert Rc<RefCell<Holon>> → Holon
            keyed_index: self.keyed_index.clone(),
        }
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
        self.holons.extend(pool.holons.into_iter().map(|h| Rc::new(RefCell::new(h))));
        self.keyed_index.extend(pool.keyed_index);
    }

    /// Inserts a new Holon into the pool and returns its index.
    ///
    /// # Arguments
    /// - `holon` - The Holon to be inserted.
    ///
    /// # Returns
    /// - `usize` representing the index where the Holon was inserted.
    pub fn insert_holon(&mut self, holon: Holon) -> usize {
        let index = self.holons.len();
        let rc_holon = Rc::new(RefCell::new(holon));

        self.holons.push(rc_holon);

        if let Ok(Some(key)) = self.holons[index].borrow().get_key() {
            self.keyed_index.insert(key, index);
        }

        index
    }
    /// Returns the number of Holons in the pool.
    ///
    /// # Returns
    /// - `usize` representing the total count of Holons in the pool.
    pub fn len(&self) -> usize {
        self.holons.len()
    }

    /// Checks if a given index is valid within the HolonPool.
    ///
    /// # Arguments
    /// - `index` - The index to check.
    ///
    /// # Returns
    /// - `true` if the index is valid, `false` otherwise.
    pub fn is_valid_index(&self, index: usize) -> bool {
        index < self.holons.len()
    }
}
