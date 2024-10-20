use std::fmt;
use std::rc::Rc;
use hdk::prelude::*;

use shared_types_holon::{HolonId, MapString, PropertyName, PropertyValue};

use crate::context::HolonsContext;
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::relationship::{RelationshipMap, RelationshipName};
use crate::smart_reference::SmartReference;
use crate::staged_reference::StagedReference;

// If I can operate directly on HolonReferences as if they were Holons, I don't need this Trait
// pub trait HolonReferenceFns {
//     fn get_rc_holon(&self) -> Result<Rc<RefCell<Holon>>, HolonError>;
// }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// HolonReference provides a general way to access Holons without having to know whether they are in a read-only
/// state (and therefore owned by the CacheManager) or being staged for creation/update (and therefore owned by the
/// CommitManager).
///
/// HolonReference also hides whether the referenced holon is in the local space or an external space
pub enum HolonReference {
    Staged(StagedReference),
    Smart(SmartReference),
}

impl fmt::Display for HolonReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HolonReference::Staged(staged_ref) => {
                write!(f, "Staged({})", staged_ref.holon_index)
            }
            HolonReference::Smart(smart_ref) => {
                write!(f, "Smart({})", smart_ref.get_holon_id_no_context())
            }
        }
    }
}

pub trait HolonGettable {

    fn get_holon_id(
        &self,
        context: &HolonsContext,
    ) -> Result<Option<HolonId>, HolonError>;

    /// This method gets a property value for a property whose descriptor is identified
    /// by `property_id`.
    fn get_property_value_by_descriptor(
        &self, // the source holon
        context: &HolonsContext,
        property_descriptor: &HolonReference,
    ) -> Result<PropertyValue, HolonError>;


    /// This function returns the primary key value for the holon or None if there is no key value
    /// for this holon (NOTE: Not all holon types have defined keys.)
    /// If the holon has a key, but it cannot be returned as a MapString, this function
    /// returns a HolonError::UnexpectedValueType.
    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError>;

    // fn query_relationship(&self, context: HolonsContext, relationship_name: RelationshipName, query_spec: Option<QuerySpec>-> SmartCollection;

    /// In this method, &self is either a HolonReference, StagedReference, SmartReference or Holon
    /// that represents the source holon, whose related holons are being requested.
    /// `relationship_name`, if provided, indicates the name of the relationship being navigated.
    /// In the future, this parameter will be replaced with an optional reference to the
    /// RelationshipDescriptor for this relationship. If None, then all holons related to the source
    /// holon across all of its relationships are retrieved. This method populates the cached source
    /// holon's HolonCollection for the specified relationship if one is provided.
    /// If relationship_name is None, the source holon's HolonCollections are populated for all
    /// relationships that have related holons.
    fn get_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError>;
}


impl HolonGettable for HolonReference {

    fn get_holon_id(&self,   context: &HolonsContext) -> Result<Option<HolonId>, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_holon_id(context),
            HolonReference::Staged(staged_reference) => staged_reference.get_holon_id(context),
            // Err(HolonError::HolonNotFound("HolonId not yet assigned for Staged Holons".to_string()))
        }
    }

    fn get_property_value_by_descriptor(
        &self,
        context: &HolonsContext,
        property_descriptor: &HolonReference,
    ) -> Result<PropertyValue, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference
                .get_property_value_by_descriptor(context, property_descriptor),
            HolonReference::Staged(staged_reference) => staged_reference
                .get_property_value_by_descriptor(context, property_descriptor),
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

impl HolonReference {
    pub fn clone_reference(&self) -> HolonReference {
        match self {
            HolonReference::Smart(smart_ref) => HolonReference::Smart(smart_ref.clone_reference()),
            HolonReference::Staged(staged_ref) => {
                HolonReference::Staged(staged_ref.clone_reference())
            }
        }
    }
    pub fn from_holon_id(holon_id: HolonId) -> Self {
        HolonReference::Smart(
            SmartReference::new(
            holon_id,
            None,
            )
        )
    }
    /// The method returns the HolonDescriptor for self's referenced holon
    fn get_descriptor(
        &self,
        context: &HolonsContext,
    ) -> Result<HolonReference, HolonError> {
        let relationship_name = RelationshipName(MapString("DESCRIBED_BY".to_string()));
        let collection = self
            .get_related_holons(context, &relationship_name )?;
        let members = collection.get_members();
        match members.len() {
            0 => Err(HolonError::NoDescriptor(format!(
                "Holon with key {:?} has NO HolonDescriptors",
                self.get_key(context)))),
            1 => Ok(members[0].clone()),
            _ => Err(HolonError::Misc(format!(
                "Holon with key {:?} has >1 HolonDescriptors",
                self.get_key(context)
            )))

        }


    }
    /// This method is provided for backwards compatibility. It accepts a PropertyName parameter and
    /// does a lookup via this holon's HolonDescriptor to get a HolonReference to the property's
    /// PropertyDescriptor and then delegates the call to `get_property_value_by_descriptor`.
    pub fn get_property_value(
        &self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        match self {
            HolonReference::Smart(smart_reference) => smart_reference.get_property_value(context, property_name),
            HolonReference::Staged(staged_reference) => staged_reference.get_property_value(context, property_name),
        }

    }
    /// Helper method that searches the source holon's descriptor's properties and either
    /// returns a reference to the PropertyDescriptor whose name matches property_name or one of
    /// the following HolonErrors:
    /// * HolonError::NoSuchProperty -- No entry for the specified property name in the source holon's property descriptors
    /// * HolonError::NoDescriptor -- Source holon (self) does not have a HolonDescriptor
    ///
    pub fn get_property_descriptor_by_name(
        &self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<HolonReference, HolonError> {
        // 1. Get a HolonReference to the HolonDescriptor for self
        let holon_descriptor = self.get_descriptor(context)?;

        // 2. Get the related holons for the "PROPERTIES" relationship
        let relationship_name = RelationshipName(MapString("PROPERTIES".to_string()));
        let collection = holon_descriptor.get_related_holons(
            context,
            &relationship_name)?;

        // 3. Retrieve the property descriptor reference by key (property_name)
        let property_name_string = MapString(property_name.to_string());
        let property_descriptor_result = collection.get_by_key(&property_name_string);

        // 4. Match on the result of `get_by_key`
        match property_descriptor_result {
            Ok(Some(property_descriptor_ref)) => Ok(property_descriptor_ref),  // Found the property descriptor
            Ok(None) => Err(HolonError::NoSuchProperty(property_name.to_string())),  // No entry for the specified property name
            Err(error) => Err(error),  // Error while retrieving the descriptor

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


//
// pub trait HolonPropertyMutators {
//         /// This method assigns a value to the property identified by `property_id`
//     /// It does NOT check if `property_id` is a valid property for the owner of this property map.
//     /// Such validation checks are the owner's responsibility
//     fn with_property_value_by_id(
//         &mut self,
//         property_id: &HolonId,
//         value: PropertyValue
//     ) -> &mut Self;
//
//
//     /// This method is provided for backwards compatibility. It accepts a PropertyName parameter and
//     /// does a lookup via this holon's HolonDescriptor to find the PropertyDescriptorId and then
//     /// delegates the call to `with_property_value_by_id`.
//     #[deprecated]
//     fn with_property_value(&mut self,
//                            _context: &HolonsContext,
//                            _property_name: &PropertyName,
//                            _value: PropertyValue
//     ) -> Result<&mut Self, HolonError>;
// }
//
// impl HolonPropertyMutators for HolonPropertyMap {
//
//     fn with_property_value_by_id(
//         &mut self,
//         property_id: HolonReference,
//         value: PropertyValue
//     ) -> &mut Self {
//         self.insert(property_id, value);
//         self
//     }
//
//     fn with_property_value(&mut self,
//                            _context: &HolonsContext,
//                            _property_name: &PropertyName,
//                            _value: PropertyValue
//     ) -> Result<&mut Self, HolonError> {
//         // Implementing this depends on being able to find and query the CoreSchema object
//         // let schema = context.get_core_schema();
//         // let descriptor_id = schema.get_related_holon_by_key(property_name)?;
//         // Ok(with_property_value_by_id(self, descriptor_id, value))
//         todo!()
//     }
// }

