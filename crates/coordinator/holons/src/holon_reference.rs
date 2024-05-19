use hdk::prelude::*;

use shared_types_holon::{HolonId, MapString, PropertyName, PropertyValue};

use crate::context::HolonsContext;
use crate::holon::HolonFieldGettable;
use crate::holon_error::HolonError;
use crate::relationship::{RelationshipMap, RelationshipName};
use crate::smart_link_manager::SmartLinkInput;
use crate::smart_link_manager::*;
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
    Staged(StagedReference),
    Smart(SmartReference),
}

impl HolonFieldGettable for HolonReference {
    fn get_property_value(
        &mut self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => {
                smart_reference.get_property_value(context, property_name)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.get_property_value(context, property_name)
            }
        }
    }

    fn get_key(&mut self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_key(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_key(context),
        }
    }
}

impl HolonReference {
    pub fn get_relationship_map(
        &mut self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_relationship_map(context),
            HolonReference::Staged(staged_reference) => {
                staged_reference.get_relationship_map(context)
            }
        }
    }
    pub fn clone_reference(&self) -> HolonReference {
        match self {
            HolonReference::Smart(smart_ref) => HolonReference::Smart(smart_ref.clone_reference()),
            HolonReference::Staged(staged_ref) => {
                HolonReference::Staged(staged_ref.clone_reference())
            }
        }
    }

    /// Commit on HolonReference persists the reference as a SmartLink in the persistent store.
    /// * If it's variant is Staged, then first commits() on its referenced holon and then uses
    /// its holon_id as the to_address for the SmartLink
    /// *  If it's variant is SmartReference, it just needs to create the SmartLink
    pub fn commit(
        &self,
        context: &HolonsContext,
        source_id: HolonId,
        relationship_name: RelationshipName,
    ) -> Result<(), HolonError> {
        match self {
            HolonReference::Staged(staged_reference) => {
                let target_holon = staged_reference.commit(context)?;
                let input = SmartLinkInput {
                    from_address: source_id,
                    to_address: target_holon.get_id().unwrap(),
                    relationship_descriptor: relationship_name,
                };
                create_smart_link(input)
            }
            HolonReference::Smart(smart_reference) => {
                let input = SmartLinkInput {
                    from_address: source_id,
                    to_address: smart_reference.clone().holon_id,
                    relationship_descriptor: relationship_name,
                };
                create_smart_link(input)
            }
        }
    }
}
