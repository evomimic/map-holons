
use crate::holon::{HolonFieldGettable};
use crate::holon_errors::HolonError;
use hdk::prelude::*;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{MapString, PropertyValue};
use crate::context::HolonsContext;
use crate::relationship::RelationshipMap;
use crate::smart_reference::SmartReference;
use crate::staged_reference::StagedReference;
// If I can operate directly on HolonReferences as if they were Holons, I don't need this Trait
// pub trait HolonReferenceFns {
//     fn get_rc_holon(&self) -> Result<Rc<RefCell<Holon>>, HolonError>;
// }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HolonReference {
    Smart(SmartReference),
    Staged(StagedReference),
}

impl HolonFieldGettable for HolonReference {

    fn get_property_value(&mut self, context: &HolonsContext, property_name: &PropertyName) -> Result<PropertyValue, HolonError> {
       match self {
           HolonReference::Smart(smart_reference) => smart_reference.get_property_value(context, property_name),
           HolonReference::Staged(staged_reference) => staged_reference.get_property_value(context, property_name),
       }

    }

    fn get_relationship_map(&mut self, context: &HolonsContext,) -> Result<RelationshipMap, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_relationship_map(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_relationship_map(context),
        }
    }

    fn get_key(&mut self, context: &HolonsContext,) -> Result<Option<MapString>,HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_key(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_key(context),
        }
    }
}

