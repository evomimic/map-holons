use std::rc::Rc;
use serde::{Serialize, Deserialize};


use crate::core_shared_objects::{
    holon::{holon_utils::EssentialHolonContent, state::AccessType},
    HolonCollection, RelationshipName, TransientHolon
};
use crate::reference_layer::{
    HolonsContextBehavior, ReadableHolon, SmartReference, StagedReference, TransientReference,
};
use base_types::MapString;
use core_types::{HolonError, HolonId};
use integrity_core_types::{PropertyName, PropertyValue};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
/// HolonReference provides a general way to access Holons without having to know whether they are in a read-only
/// state (and therefore owned by the CacheManager) or being staged for creation/update (and therefore owned by the
/// Nursery).
///
/// HolonReference also hides whether the referenced holon is in the local space or an external space
pub enum HolonReference {
    Transient(TransientReference),
    Staged(StagedReference),
    Smart(SmartReference),
}

impl ReadableHolon for HolonReference {
    fn clone_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientHolon, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.clone_holon(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.clone_holon(context),
            HolonReference::Smart(smart_reference) => smart_reference.clone_holon(context),
        }
    }

    fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.essential_content(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.essential_content(context),
            HolonReference::Smart(smart_reference) => smart_reference.essential_content(context),
        }
    }

    fn get_holon_id(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.get_holon_id(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.get_holon_id(context),
            HolonReference::Smart(smart_reference) => smart_reference.get_holon_id(context),
        }
    }

    fn get_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => transient_reference.get_key(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_key(context),
            HolonReference::Smart(smart_reference) => smart_reference.get_key(context),
        }
    }

    fn get_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.get_predecessor(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.get_predecessor(context),
            HolonReference::Smart(smart_reference) => smart_reference.get_predecessor(context),
        }
    }

    fn get_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.get_property_value(context, property_name)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.get_property_value(context, property_name)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.get_property_value(context, property_name)
            }
        }
    }

    fn get_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.get_related_holons(context, relationship_name)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.get_related_holons(context, relationship_name)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.get_related_holons(context, relationship_name)
            }
        }
    }

    fn get_versioned_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.get_versioned_key(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.get_versioned_key(context),
            HolonReference::Smart(smart_reference) => smart_reference.get_versioned_key(context),
        }
    }

    fn is_accessible(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.is_accessible(context, access_type)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.is_accessible(context, access_type)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.is_accessible(context, access_type)
            }
        }
    }
}

/// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining lineage to the Holon its cloned from.
impl HolonReference {
    // pub fn stage_new_from_clone(
    //     &self,
    //     context: &dyn HolonsContextBehavior,
    // ) -> Result<StagedReference, HolonError> {
    //     let cloned_holon = match self {
    //         HolonReference::Staged(staged_reference) => {
    //             // Get a clone from the rc_holon in the commit_manager
    //             staged_reference.stage_new_from_clone_deprecated(context)?
    //         }
    //         HolonReference::Smart(smart_reference) => {
    //             // Get a clone from the rc_holon in the cache_manager
    //             smart_reference.stage_new_from_clone_deprecated(context)?
    //         }
    //     };
    //
    //     let cloned_staged_reference = {
    //         // Mutably borrow the commit_manager
    //         let space_manager = context.get_space_manager();
    //         // Stage the clone
    //         space_manager.stage_new_holon(cloned_holon)?
    //     };
    //
    //     // Reset the PREDECESSOR to None
    //     cloned_staged_reference.with_predecessor(context, None)?;
    //
    //     Ok(cloned_staged_reference)
    // }

    //
    /// Creates a `HolonReference` wrapping a `SmartReference` for the given `HolonId`.
    pub fn from_id(holon_id: HolonId) -> HolonReference {
        HolonReference::Smart(SmartReference::new_from_id(holon_id))
    }
    /// Creates a `HolonReference::Staged` variant from a `StagedReference`.
    pub fn from_staged(staged: StagedReference) -> Self {
        HolonReference::Staged(staged)
    }

    /// Creates a `HolonReference::Smart` variant from a `SmartReference`.
    pub fn from_smart(smart: SmartReference) -> Self {
        HolonReference::Smart(smart)
    }

    pub fn get_descriptor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                let relationship_name = RelationshipName(MapString("DESCRIBED_BY".to_string()));
                // let relationship_name = CoreSchemaRelationshipTypeName::DescribedBy.to_string();
                let collection =
                    transient_reference.get_related_holons(context, &relationship_name)?;
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
        }
    }

    pub fn get_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.get_predecessor(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.get_predecessor(context),
            HolonReference::Smart(smart_reference) => smart_reference.get_predecessor(context),
        }
    }
}
