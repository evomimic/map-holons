use hdk::prelude::*;

use shared_types_holon::{MapString, PropertyValue};
use shared_types_holon::holon_node::PropertyName;

use crate::context::HolonsContext;
use crate::holon::HolonFieldGettable;
use crate::holon_errors::HolonError;
use crate::relationship::RelationshipMap;
use crate::smart_reference::SmartReference;
use crate::staged_reference::StagedReference;

// If I can operate directly on HolonReferences as if they were Holons, I don't need this Trait
// pub trait HolonReferenceFns {
//     fn get_rc_holon(&self) -> Result<Rc<RefCell<Holon>>, HolonError>;
// }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
/// HolonReference provides a general way to access Holons without having to know whether they are in a read-only
/// state (and therefore owned by the CacheManager) or being staged for creation/update (and therefore owned by the
/// CommitManager).
///
/// HolonReference also hides whether the referenced holon is in the local space or an external space
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

    fn get_key(&mut self, context: &HolonsContext,) -> Result<Option<MapString>,HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_key(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_key(context),
        }
    }
}

impl HolonReference {
    pub fn get_relationship_map(&mut self, context: &HolonsContext,) -> Result<RelationshipMap, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_relationship_map(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_relationship_map(context),
        }
    }
    pub fn clone_reference(&self) -> HolonReference {
        match self {
            HolonReference::Smart(smart_ref) => HolonReference::Smart(smart_ref.clone_reference()),
            HolonReference::Staged(staged_ref) => HolonReference::Staged(staged_ref.clone_reference()),
        }
    }

}

