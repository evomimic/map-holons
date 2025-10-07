use serde::{Deserialize, Serialize};
use type_names::relationship_names::CoreRelationshipTypeName;

use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::{
        holon::{holon_utils::EssentialHolonContent, state::AccessType},
        HolonCollection,
    },
    reference_layer::{
        HolonsContextBehavior, ReadableHolon, SmartReference, StagedReference, TransientReference,
    },
    RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyName, PropertyValue, RelationshipName,
};
use std::sync::{Arc, RwLock};

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

/// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining lineage to the Holon its cloned from.
impl HolonReference {
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
        self.is_accessible(context, AccessType::Read)?;
        match self {
            HolonReference::Transient(transient_reference) => {
                let collection_arc = transient_reference
                    .related_holons(context, CoreRelationshipTypeName::DescribedBy)?;
                let collection = collection_arc.read().map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire read lock on holon collection: {}",
                        e
                    ))
                })?;
                collection.is_accessible(AccessType::Read)?;
                let members = collection.get_members();
                if members.len() > 1 {
                    return Err(HolonError::Misc(format!(
                        "related_holons for DESCRIBED_BY returned multiple members: {:#?}",
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
                let collection_arc = staged_reference
                    .related_holons(context, CoreRelationshipTypeName::DescribedBy)?;
                let collection = collection_arc.read().map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire read lock on holon collection: {}",
                        e
                    ))
                })?;
                collection.is_accessible(AccessType::Read)?;
                let members = collection.get_members();
                if members.len() > 1 {
                    return Err(HolonError::Misc(format!(
                        "related_holons for DESCRIBED_BY returned multiple members: {:#?}",
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
                let collection_arc = smart_reference.related_holons(
                    context,
                    CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                )?;
                let collection = collection_arc.read().map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire read lock on holon collection: {}",
                        e
                    ))
                })?;
                collection.is_accessible(AccessType::Read)?;
                let members = collection.get_members();
                if members.len() > 1 {
                    return Err(HolonError::Misc(format!(
                        "related_holons for DESCRIBED_BY returned multiple members: {:#?}",
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

    pub fn predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.predecessor(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.predecessor(context),
            HolonReference::Smart(smart_reference) => smart_reference.predecessor(context),
        }
    }
}

impl ReadableHolonImpl for HolonReference {
    fn clone_holon_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.clone_holon_impl(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.clone_holon_impl(context),
            HolonReference::Smart(smart_reference) => smart_reference.clone_holon_impl(context),
        }
    }

    fn all_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.all_related_holons_impl(context)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.all_related_holons_impl(context)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.all_related_holons_impl(context)
            }
        }
    }

    fn essential_content_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.essential_content_impl(context)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.essential_content_impl(context)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.essential_content_impl(context)
            }
        }
    }

    fn holon_id_impl(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.holon_id_impl(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.holon_id_impl(context),
            HolonReference::Smart(smart_reference) => smart_reference.holon_id_impl(context),
        }
    }

    fn into_model_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        match self {
            Self::Transient(reference) => reference.into_model_impl(context),
            Self::Staged(reference) => reference.into_model_impl(context),
            Self::Smart(reference) => reference.into_model_impl(context),
        }
    }

    fn is_accessible_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.is_accessible_impl(context, access_type)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.is_accessible_impl(context, access_type)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.is_accessible_impl(context, access_type)
            }
        }
    }

    fn key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => transient_reference.key_impl(context),
            HolonReference::Staged(staged_reference) => staged_reference.key_impl(context),
            HolonReference::Smart(smart_reference) => smart_reference.key_impl(context),
        }
    }

    fn predecessor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.predecessor_impl(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.predecessor_impl(context),
            HolonReference::Smart(smart_reference) => smart_reference.predecessor_impl(context),
        }
    }

    fn property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.property_value_impl(context, property_name)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.property_value_impl(context, property_name)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.property_value_impl(context, property_name)
            }
        }
    }

    fn related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.related_holons_impl(context, relationship_name)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.related_holons_impl(context, relationship_name)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.related_holons_impl(context, relationship_name)
            }
        }
    }

    fn summarize_impl(&self, context: &dyn HolonsContextBehavior) -> Result<String, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.summarize_impl(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.summarize_impl(context),
            HolonReference::Smart(smart_reference) => smart_reference.summarize_impl(context),
        }
    }

    fn versioned_key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.versioned_key_impl(context)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.versioned_key_impl(context)
            }
            HolonReference::Smart(smart_reference) => smart_reference.versioned_key_impl(context),
        }
    }

    fn essential_content_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.essential_content_impl(context)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.essential_content_impl(context)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.essential_content_impl(context)
            }
        }
    }

    fn summarize_impl(&self, context: &dyn HolonsContextBehavior) -> Result<String, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.summarize_impl(context)
            }
            HolonReference::Staged(staged_reference) => staged_reference.summarize_impl(context),
            HolonReference::Smart(smart_reference) => smart_reference.summarize_impl(context),
        }
    }

    fn into_model_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        match self {
            Self::Transient(reference) => reference.into_model_impl(context),
            Self::Staged(reference) => reference.into_model_impl(context),
            Self::Smart(reference) => reference.into_model_impl(context),
        }
    }

    fn is_accessible_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.is_accessible_impl(context, access_type)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.is_accessible_impl(context, access_type)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.is_accessible_impl(context, access_type)
            }
        }
    }
}

impl WritableHolonImpl for HolonReference {
    fn add_related_holons_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.add_related_holons_impl(context, relationship_name, holons)?;
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.add_related_holons_impl(context, relationship_name, holons)?;
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.add_related_holons_impl(context, relationship_name, holons)?;
            }
        }

        Ok(self)
    }

    fn remove_related_holons_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.remove_related_holons_impl(
                    context,
                    relationship_name,
                    holons,
                )?;
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.remove_related_holons_impl(context, relationship_name, holons)?;
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.remove_related_holons_impl(context, relationship_name, holons)?;
            }
        }

        Ok(self)
    }

    fn with_property_value_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        info!("Entered HolonReference::with_property_value_impl");
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.with_property_value_impl(context, property, value)?;
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.with_property_value_impl(context, property, value)?;
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.with_property_value_impl(context, property, value)?;
            }
        }

        Ok(self)
    }

    fn remove_property_value_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        name: PropertyName,
    ) -> Result<&mut Self, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.remove_property_value_impl(context, name)?;
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.remove_property_value_impl(context, name)?;
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.remove_property_value_impl(context, name)?;
            }
        }
        Ok(self)
    }

    fn with_descriptor_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.with_descriptor_impl(context, descriptor_reference)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.with_descriptor_impl(context, descriptor_reference)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.with_descriptor_impl(context, descriptor_reference)
            }
        }
    }

    fn with_predecessor_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        predecessor_reference_option: Option<HolonReference>,
    ) -> Result<(), HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.with_predecessor_impl(context, predecessor_reference_option)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.with_predecessor_impl(context, predecessor_reference_option)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.with_predecessor_impl(context, predecessor_reference_option)
            }
        }
    }
}
