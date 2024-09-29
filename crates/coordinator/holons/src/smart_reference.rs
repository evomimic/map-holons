use std::cell::RefCell;
use std::rc::Rc;

use derive_new::new;
use hdk::prelude::*;

use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{HolonId, MapString, PropertyMap, PropertyValue};

use crate::commit_manager::{self, StagedIndex};
use crate::context::HolonsContext;
use crate::holon::{AccessType, EssentialHolonContent, Holon};
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::holon_reference::{HolonGettable, HolonReference};
use crate::relationship::{RelationshipMap, RelationshipName};
use crate::staged_reference::StagedReference;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartReference {
    //holon_space_id: Option<SpaceId>
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl SmartReference {
    pub fn clone_reference(&self) -> SmartReference {
        SmartReference {
            holon_id: self.holon_id.clone(),
            smart_property_values: self.smart_property_values.clone(),
        }
    }

    pub fn essential_content(
        &self,
        context: &HolonsContext,
    ) -> Result<EssentialHolonContent, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.essential_content()
    }

    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }

    pub fn get_predecessor(
        &self,
        context: &HolonsContext,
    ) -> Result<Option<HolonReference>, HolonError> {
        let relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));
        // let relationship_name = CoreSchemaRelationshipTypeName::DescribedBy.to_string();
        let collection = self.get_related_holons(context, &relationship_name)?;
        collection.is_accessible(AccessType::Read)?;
        let members = collection.get_members();
        if members.len() > 1 {
            return Err(HolonError::Misc(format!(
                "get_related_holons for PREDECESSOR returned multiple members: {:#?}",
                members
            )));
        }
        if members.is_empty() {
            Ok(None)
        } else {
            Ok(Some(members[0].clone()))
        }
    }

    pub fn get_property_map(&self, context: &HolonsContext) -> Result<PropertyMap, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        Ok(holon_refcell.property_map.clone())
    }

    // Private function for getting a mutable reference from the context
    fn get_rc_holon(&self, context: &HolonsContext) -> Result<Rc<RefCell<Holon>>, HolonError> {
        debug!("Entered: get_rc_holon, trying to get the cache_manager");
        let cache_manager = match context.cache_manager.try_borrow() {
            Ok(cache_manager) => cache_manager,
            Err(borrow_error) => {
                error!(
                    "Failed to borrow cache_manager, it is already borrowed mutably: {:?}",
                    borrow_error
                );
                return Err(HolonError::FailedToBorrow(format!("{:?}", borrow_error)));
            }
        };
        debug!("Cache manager borrowed successfully");

        let rc_holon = cache_manager.get_rc_holon(&self.holon_id)?;
        debug!("Got a reference to rc_holon from the cache manager");

        Ok(rc_holon)
    }

    pub fn get_relationship_map(
        &self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        Ok(holon_refcell.relationship_map.clone())
    }

    pub fn get_smart_properties(&self) -> Option<PropertyMap> {
        self.smart_property_values.clone()
    }

    /// Stages a new version of an existing holon for update, retaining the linkage to the holon version it is derived from by creating a PREDECESSOR relationship.
    pub fn new_version(&self, context: &HolonsContext) -> Result<StagedReference, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let new_holon = rc_holon.borrow().new_version()?;

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
        let staged_reference = commit_manager.stage_new_holon(new_holon)?;

        // Set PREDECESSOR and return StagedReference
        staged_reference.with_predecessor(context, Some(HolonReference::Smart(self.clone())))
    }

    /// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining lineage to the Holon its cloned from.
    pub fn stage_new_from_clone(&self, context: &HolonsContext) -> Result<Holon, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.borrow().clone_holon()?;

        Ok(holon)
    }
}

impl HolonGettable for SmartReference {
    /// This function gets the value for the specified property name
    /// It will attempt to get it from the smart_property_values map first to avoid having to
    /// retrieve the underlying holon. But, failing that, it will do a get_rc_holon from the cache manager in the context.
    ///
    /// Possible Errors:
    /// This function returns an EmptyFiled error if no value is found for the specified property
    /// Or (less likely) an InvalidHolonReference
    fn get_property_value(
        &self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        // Check if the property value is available in smart_property_values
        if let Some(smart_map) = &self.smart_property_values {
            if let Some(value) = smart_map.get(property_name) {
                return Ok(value.clone());
            }
        }

        // Get rc_holon from HolonCacheManager
        let holon = self.get_rc_holon(context)?;
        let prop_val = holon.borrow().get_property_value(property_name)?;
        Ok(prop_val)
    }

    /// This function extracts the key from the smart_property_values,
    ///
    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        return if let Some(smart_prop_vals) = self.smart_property_values.clone() {
            let key_option = smart_prop_vals.get(&PropertyName(MapString("key".to_string())));
            if let Some(key) = key_option {
                Ok(Some(MapString(key.into())))
            } else {
                let holon = self.get_rc_holon(context)?;
                let key = holon.borrow().get_key()?;
                Ok(key)
            }
        } else {
            let holon = self.get_rc_holon(context)?;
            let key = holon.borrow().get_key()?;
            Ok(key)
        };
    }
    // pub fn get_key(&self) -> Result<Option<MapString>, HolonError> {
    //     Ok(self
    //         .smart_property_values
    //         .as_ref()
    //         .and_then(|prop_map| prop_map.get(&PropertyName(MapString("key".to_string()))))
    //         .and_then(|prop_value| match prop_value {
    //             BaseValue::StringValue(s) => Some(s.clone()),
    //             _ => None,
    //         }))
    // }
    // fn get_key(&self) -> Result<Option<MapString>, HolonError> {
    //     return if let Some(smart_prop_vals) = self.smart_property_values.clone() {
    //         let key_option = smart_prop_vals.get(&PropertyName(MapString("key".to_string())));
    //         if let Some(key) = key_option {
    //             Ok(Some(MapString(key.into())))
    //         } else {
    //             Ok(None)
    //         }
    //     }
    //     Ok(None)
    //
    //
    // }
    fn get_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let map = {
            let mut holon_refcell = holon.borrow_mut();
            holon_refcell.get_related_holons(relationship_name)?.clone()
        };
        Ok(map)
    }

    // fn get_related_holons(
    //     &self,
    //     context: &HolonsContext,
    //     relationship_name: &RelationshipName,
    // ) -> Result<&HolonCollection, HolonError> {
    //     let holon = self.get_rc_holon(context)?;
    //     let map = holon
    //         .borrow()
    //         .get_related_holons(relationship_name)?;
    //     Ok(map)
    // }

    // Populates the cached source holon's HolonCollection for the specified relationship if one is provided.
    // If relationship_name is None, the source holon's HolonCollections are populated for all relationships that have related holons.
    // fn get_related_holons(
    //     &self,
    //     context: &HolonsContext,
    //     relationship_name: Option<RelationshipName>,
    // ) -> Result<RelationshipMap, HolonError> {
    //     let holon = self.get_rc_holon(context)?;
    //     let map = holon
    //         .borrow()
    //         .get_related_holons(context, relationship_name)?;
    //     Ok(map)
    // }
}
