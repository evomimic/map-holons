use crate::shared_objects_layer::nursery_access::NurseryAccess;
use crate::shared_objects_layer::{Holon, HolonError};
use shared_types_holon::MapString;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nursery {
    staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
    keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}

impl Nursery {
    /// Creates a new, empty `Nursery`.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { staged_holons: Vec::new(), keyed_index: BTreeMap::new() }
    }

    /// Initializes a `Nursery` from a set of staged holons and a keyed index.
    ///
    /// # Arguments
    ///
    /// * `staged_holons` - A vector of staged holons.
    /// * `keyed_index` - A map of keys to indices into the staged_holons vector
    ///
    /// # Returns
    ///
    /// A `Nursery` instance initialized with the provided holons and keyed index.
    pub fn new_from_stage(
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
    ) -> Self {
        Self { staged_holons, keyed_index }
    }
    /// Clears all staged holons and their keyed indices from the nursery.
    pub fn clear_stage(&mut self) {
        self.staged_holons.clear();
        self.keyed_index.clear();
    }

    /// Stages a new holon and optionally updates the keyed index.
    ///
    /// # Arguments
    /// * `holon` - A reference to the holon to be staged.
    ///
    /// # Returns
    /// The index of the staged holon in the nursery.
    pub fn stage_holon(&mut self, holon: &Holon) -> usize {
        // Create a new Rc<RefCell<Holon>> for the holon and add it to the staged_holons
        let rc_holon = Rc::new(RefCell::new(holon.clone()));
        let staged_index = self.staged_holons.len();
        self.staged_holons.push(rc_holon);

        // If the holon has a key, update the keyed index
        if let Ok(Some(key)) = holon.get_key() {
            self.keyed_index.insert(key, staged_index);
        }

        staged_index
    }

    /// Finds a holon by its key and returns its index.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// `Ok(usize)` containing the index if the key exists, or an `Err` if the key is not found.
    pub fn get_index_by_key(&self, key: &MapString) -> Result<usize, HolonError> {
        self.keyed_index.get(key).cloned().ok_or_else(|| {
            HolonError::HolonNotFound(format!("No staged holon found for key: {}", key))
        })
    }

    /// Retrieves a holon by its index.
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the holon to retrieve.
    ///
    /// # Returns
    ///
    /// `Ok(Rc<RefCell<Holon>>)` containing the holon if the index is valid, or an `Err` if the index is out of range.
    pub fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.staged_holons
            .get(index)
            .cloned()
            .ok_or_else(|| HolonError::IndexOutOfRange(format!("No holon at index {}", index)))
    }

    /// Returns a copy of the Nursery's keyed_index BTreeMap.
    /// CAUTION: This method is ONLY intended for use by the HolonSpaceManager
    /// # Returns
    ///
    /// A reference to the vector of staged holons.
    pub fn get_keyed_index(&self) -> BTreeMap<MapString, usize> {
        self.keyed_index.clone()
    }

    /// Returns a reference to all staged holons.
    /// CAUTION: This method is ONLY intended for use by the HolonSpaceManager
    ///
    /// # Returns
    ///
    /// A reference to the vector of staged holons.
    pub fn get_staged_holons(&self) -> &Vec<Rc<RefCell<Holon>>> {
        &self.staged_holons
    }

    /// Checks if an index is valid within the `staged_holons` vector.
    ///
    /// # Arguments
    ///
    /// * `index` - The index to check.
    ///
    /// # Returns
    ///
    /// `true` if the index is valid, `false` otherwise.
    #[allow(dead_code)]
    pub fn is_valid_index(&self, index: usize) -> bool {
        index < self.staged_holons.len()
    }
    // pub fn new() -> Nursery {
    //     Nursery { staged_holons: Vec::new(), keyed_index: BTreeMap::new() }
    // }
    // pub fn new_from_stage(
    //     staged_holons: Vec<Rc<RefCell<Holon>>>,
    //     keyed_index: BTreeMap<MapString, usize>,
    // ) -> Self {
    //     Self { staged_holons, keyed_index }
    // }
    //
    // /// Finds a holon by its key and returns its index.
    // pub fn get_index_by_key(&self, key: &MapString) -> Result<usize, HolonError> {
    //     self.keyed_index
    //         .get(key)
    //         .cloned()
    //         .ok_or_else(|| HolonError::HolonNotFound(format!("No holon found for key: {}", key)))
    // }
    //
    // /// Stages a new holon and updates the keyed index if the holon has a key.
    // pub fn stage_holon(&mut self, holon: Holon) -> Result<usize, HolonError> {
    //     let rc_holon = Rc::new(RefCell::new(holon));
    //     let staged_index = self.staged_holons.len();
    //     self.staged_holons.push(rc_holon.clone());
    //
    //     if let Some(key) = rc_holon.borrow().get_key()? {
    //         self.keyed_index.insert(key, staged_index);
    //     }
    //
    //     Ok(staged_index)
    // }
}
impl NurseryAccess for Nursery {
    /// Finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon matching the given index.
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the holon in the staged holons vector.
    ///
    /// # Returns
    ///
    /// A `Result` containing the shared reference to the staged holon or a `HolonError` if the index is out of range.
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.staged_holons.get(index).cloned().ok_or_else(|| {
            HolonError::IndexOutOfRange(format!(
                "Invalid index: {}. No staged holon at this position.",
                index
            ))
        })
    }

    // fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
    //     if index < self.staged_holons.len() {
    //         let holon_ref = &self.staged_holons[index];
    //         // match holon_ref.try_borrow() {
    //         ////     Ok(holon) => Ok(holon),
    //         //  Err(_) => Err(HolonError::FailedToBorrow("Failed to borrow holon".into()))
    //         //}
    //         Ok(Rc::clone(holon_ref))
    //     } else {
    //         Err(HolonError::IndexOutOfRange(index.to_string()))?
    //     }
    // }
}
