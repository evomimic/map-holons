use hdk::prelude::*;
use std::rc::Rc;

use shared_types_holon::{HolonId, MapString, PropertyName, PropertyValue};

use crate::context::HolonsContext;
use crate::holon::{AccessType, EssentialHolonContent};
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::relationship::{RelationshipMap, RelationshipName};
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

pub trait HolonGettable {
    fn get_property_value(
        &self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError>;

    /// This function returns the primary key value for the holon or None if there is no key value
    /// for this holon (NOTE: Not all holon types have defined keys.)
    /// If the holon has a key, but it cannot be returned as a MapString, this function
    /// returns a HolonError::UnexpectedValueType.
    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError>;

    // fn query_relationship(&self, context: HolonsContext, relationship_name: RelationshipName, query_spec: Option<QuerySpec>-> SmartCollection;

    /// In this method, &self is either a HolonReference, StagedReference, SmartReference or Holon that represents the source holon,
    /// whose related holons are being requested. relationship_name, if provided, indicates the name of the relationship being navigated.
    /// In the future, this parameter will be replaced with an optional reference to the RelationshipDescriptor for this relationship.
    /// If None, then all holons related to the source holon across all of its relationships are retrieved.
    /// This method populates the cached source holon's HolonCollection for the specified relationship if one is provided.
    /// If relationship_name is None, the source holon's HolonCollections are populated for all relationships that have related holons.
    fn get_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError>;
}

impl HolonGettable for HolonReference {
    fn get_property_value(
        &self,
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

    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_key(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_key(context),
        }
    }

    fn get_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        match self {
            HolonReference::Smart(reference) => {
                reference.get_related_holons(context, relationship_name)
            }
            HolonReference::Staged(reference) => {
                reference.get_related_holons(context, relationship_name)
            }
        }
    }
}

/// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining lineage to the Holon its cloned from.
impl HolonReference {
    pub fn stage_new_from_clone(
        &self,
        context: &HolonsContext,
    ) -> Result<StagedReference, HolonError> {
        let cloned_holon = match self {
            HolonReference::Staged(staged_reference) => {
                // Get a clone from the rc_holon in the commit_manager
                staged_reference.stage_new_from_clone(context)?
            }
            HolonReference::Smart(smart_reference) => {
                // Get a clone from the rc_holon in the cache_manager
                smart_reference.stage_new_from_clone(context)?
            }
        };

        let cloned_staged_reference = {
            // Mutably borrow the commit_manager
            let mut commit_manager = match context.commit_manager.try_borrow_mut() {
                Ok(commit_manager) => commit_manager,
                Err(borrow_error) => {
                    error!(
                        "Failed to borrow commit_manager mutably: {:?}",
                        borrow_error
                    );
                    return Err(HolonError::FailedToBorrow(format!("{:?}", borrow_error)));
                }
            };

            // Stage the clone
            commit_manager.stage_new_holon(cloned_holon)?
        };

        // Reset the PREDECESSOR to None
        cloned_staged_reference.with_predecessor(context, None)?;

        Ok(cloned_staged_reference)
    }

    pub fn clone_reference(&self) -> HolonReference {
        match self {
            HolonReference::Smart(smart_reference) => {
                HolonReference::Smart(smart_reference.clone_reference())
            }
            HolonReference::Staged(staged_reference) => {
                HolonReference::Staged(staged_reference.clone_reference())
            }
        }
    }

    pub fn essential_content(
        &self,
        context: &HolonsContext,
    ) -> Result<EssentialHolonContent, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.essential_content(context),
            HolonReference::Staged(staged_reference) => staged_reference.essential_content(context),
        }
    }

    pub fn get_descriptor(
        &self,
        context: &HolonsContext,
    ) -> Result<Option<HolonReference>, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => {
                let relationship_name = RelationshipName(MapString("DESCRIBED_BY".to_string()));
                // let relationship_name = CoreSchemaRelationshipTypeName::DescribedBy.to_string();
                let collection = smart_reference.get_related_holons(context, &relationship_name)?;
                collection.is_accessible(AccessType::Read)?;
                let members = collection.get_members();
                if members.len() > 1 {
                    return Err(HolonError::Misc(format!(
                        "get_related_holons for DESCRIBED_BY returned multiple members: {:#?}",
                        members
                    )));
                }
                if members.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(members[0].clone()))
                }
            }
            HolonReference::Staged(staged_reference) => {
                let relationship_name = RelationshipName(MapString("DESCRIBED_BY".to_string()));
                // let relationship_name = CoreSchemaRelationshipTypeName::DescribedBy.to_string();
                let collection =
                    staged_reference.get_related_holons(context, &relationship_name)?;
                collection.is_accessible(AccessType::Read)?;
                let members = collection.get_members();
                if members.len() > 1 {
                    return Err(HolonError::Misc(format!(
                        "get_related_holons for DESCRIBED_BY returned multiple members: {:#?}",
                        members
                    )));
                }
                if members.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(members[0].clone()))
                }
            }
        }
    }

    pub fn get_holon_id(&self, context: &HolonsContext) -> Result<HolonId, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_id(),
            HolonReference::Staged(staged_reference) => staged_reference.get_id(context),
        }
    }

    pub fn get_predecessor(
        &self,
        context: &HolonsContext,
    ) -> Result<Option<HolonReference>, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_predecessor(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_predecessor(context),
        }
    }

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

    fn is_accessible(
        &self,
        context: &HolonsContext,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        HolonReference::Smart(smart_reference) => {
            smart_reference.is_accessible(context, access_type)
        }
        HolonReference::Staged(staged_reference) => {
            staged_reference.is_accessible(context, access_type)
        }
    }
    pub fn get_holon_id(&self) -> Result<HolonId, HolonError> {
        match self {

            HolonReference::Smart(smart_reference) => smart_reference.get_id(),
            HolonReference::Staged(_staged_reference) =>
                Err(HolonError::HolonNotFound("HolonId not yet assigned for Staged Holons".to_string()))
        }
    }

    // /// Commit on HolonReference persists the reference as a SmartLink for the specified
    // /// relationship and source_id
    // /// This function assumes all StagedHolons have been committed before ANY relationships. Thus,
    // /// it should be possible to get the target HolonId (i.e., to_address) from EITHER
    // /// a SmartReference or StagedReference variant.
    //
    // pub fn commit_smartlink(
    //     &self,
    //     context: &HolonsContext,
    //     source_id: HolonId,
    //     relationship_name: RelationshipName,
    // ) -> Result<(), HolonError> {
    //     debug!("Entered HolonReference::commit_smartlink");
    //     let target_id = match self {
    //         HolonReference::Smart(smart_reference) => {
    //             Ok(smart_reference.holon_id.clone())
    //         }
    //         HolonReference::Staged(staged_ref) => {
    //             debug!("Attempting to borrow commit_manager");
    //             let commit_manager = context.commit_manager.borrow();
    //             let holon = commit_manager.get_holon(staged_ref)?;
    //             debug!("Attempting to get holon_id from staged reference's holon");
    //             holon.get_id().clone()
    //         }
    //     };
    //     debug!("Got target_id {:?}",target_id.clone());
    //
    //     if let Ok(to_address) = target_id {
    //         let input = SmartLinkInput {
    //             from_address: source_id,
    //             to_address,
    //             relationship_descriptor: relationship_name,
    //         };
    //         create_smart_link(input)
    //     } else {
    //         Err(HolonError::CommitFailure("Unable to get holon_id from HolonReference".to_string()))
    //     }
    // }
}
