use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use derive_new::new;
use crate::holon::{Holon, HolonFieldGettable};
use crate::holon_errors::HolonError;
use crate::staged_reference;
use hdk::prelude::*;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{MapString, PropertyValue};
use crate::context::HolonsContext;
use crate::relationship::RelationshipName;


#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct StagedReference {
    pub rc_holon : Rc<RefCell<Holon>>,
}

impl StagedReference {
    // Method to clone the underlying Holon object
    pub fn clone_holon(&self) -> Holon {
        self.rc_holon.borrow().clone()
    }
}


impl HolonFieldGettable for StagedReference {
    fn get_property_value(&self, context: &HolonsContext, property_name: &PropertyName) -> Result<PropertyValue, HolonError> {
        let holon = self.rc_holon.borrow();
        holon.get_property_value(context, property_name)
    }

    fn get_property_names(&self, context: &HolonsContext) -> Result<Vec<PropertyName>, HolonError> {
        let holon = self.rc_holon.borrow();
        holon.get_property_names(context)
    }

    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        self.get_key(context)
    }
}


