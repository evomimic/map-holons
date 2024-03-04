use std::cell::RefCell;
use std::rc::Rc;
use derive_new::new;
use crate::holon::{Holon, HolonFieldGettable};
use crate::holon_errors::HolonError;
use hdk::prelude::*;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{MapString, PropertyValue};
use crate::context::HolonsContext;
use crate::relationship::{RelationshipMap};


#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct StagedReference {
    pub key : Option<MapString>,
    pub rc_holon : Rc<RefCell<Holon>>,
}

impl StagedReference {
    // Method to clone the underlying Holon object
    pub fn clone_holon(&self) -> Holon {
        self.rc_holon.borrow().clone()
    }
}


impl HolonFieldGettable for StagedReference {
    fn get_property_value(&mut self, _context: &HolonsContext, property_name: &PropertyName) -> Result<PropertyValue, HolonError> {
        let holon = self.rc_holon.borrow();
        holon.get_property_value( property_name)
    }

    fn get_relationship_map(&mut self, _context: &HolonsContext) -> Result<RelationshipMap, HolonError> {
        todo!()
    }

    fn get_key(&mut self, _context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        let holon = self.rc_holon.borrow();
        holon.get_key().clone()
    }
}
// Constructor function for creating from Holon Reference
pub fn from_holon(rc_holon: Rc<RefCell<Holon>>) -> Result<StagedReference, HolonError> {
    let key = rc_holon.borrow().get_key()?;

    Ok(StagedReference {
        key,
        rc_holon,
    })
}




