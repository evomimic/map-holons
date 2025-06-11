use std::{cell::RefCell, rc::Rc};

use core_types::TemporaryId;

use crate::{core_shared_objects::{holon::Holon, TransientManagerAccess}, HolonError, HolonPool};




#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransientHolonManager {
    transient_holons: Rc<RefCell<HolonPool>>, // Uses Rc<RefCell<HolonPool>> for interior mutability
}

impl TransientManagerAccess for TransientHolonManager {
    /// Retrieves a transient holon by index.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.transient_holons.borrow().get_holon_by_id(id)
    }
}