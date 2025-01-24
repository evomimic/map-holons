use crate::core_shared_objects::{Holon, HolonError, NurseryAccess};
use shared_types_holon::MapString;
use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};

pub trait NurseryAccessInternal: NurseryAccess {
    /// # CAUTION!!!
    ///
    /// **This method is ONLY intended for use by the GuestHolonService**
    ///
    /// Clears the Nursery's staged holons
    fn clear_stage(&mut self);

    /// Returns a copy of the Nursery's keyed_index BTreeMap.
    /// CAUTION: This method is ONLY intended for use by the HolonSpaceManager
    /// # Returns
    ///
    /// A reference to the vector of staged holons.
    fn get_keyed_index(&self) -> BTreeMap<MapString, usize>;

    /// Finds a holon by its key and returns its index.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to search for.
    ///
    /// # Returns
    ///
    /// `Ok(usize)` containing the index if the key exists, or an `Err` if the key is not found.
    fn get_index_by_key(&self, key: &MapString) -> Result<usize, HolonError>;

    /// Returns a clone of the staged holons vector.
    ///
    /// # Returns
    /// A clone of the `Vec<Rc<RefCell<Holon>>>` representing the staged holons.
    fn get_staged_holons(&self) -> Vec<Rc<RefCell<Holon>>>;

    /// Stages a new holon and optionally updates the keyed index.
    ///
    /// # Arguments
    /// * `holon` - A reference to the holon to be staged.
    ///
    /// # Returns
    /// The index of the staged holon in the nursery.
    fn stage_holon(&self, holon: &Holon) -> usize;
}
