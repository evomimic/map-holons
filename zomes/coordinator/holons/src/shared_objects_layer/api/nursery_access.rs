use crate::shared_objects_layer::{Holon, HolonError};
use std::{cell::RefCell, rc::Rc};

pub trait NurseryAccess {
    /// This function finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon matching the
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError>;
}
