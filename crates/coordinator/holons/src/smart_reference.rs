use std::cell::RefCell;
use std::rc::Rc;

use derive_new::new;
use hdk::prelude::*;

use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{HolonId, MapString, PropertyMap, PropertyValue};

use crate::context::HolonsContext;
use crate::holon::Holon;
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonGettable;
use crate::relationship::{RelationshipMap, RelationshipName};

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartReference {
    //holon_space_id: Option<SpaceId>
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl SmartReference {
    /// Constructor for SmartReference that takes a HolonId and sets smart_property_values to None
    pub fn new_from_id(holon_id: HolonId) -> Self {
        SmartReference { holon_id, smart_property_values: None }
    }
    pub fn clone_reference(&self) -> SmartReference {
        SmartReference {
            holon_id: self.holon_id.clone(),
            smart_property_values: self.smart_property_values.clone(),
        }
    }
    // pub fn clone_holon(&self, context: &HolonsContext)->Result<Holon, HolonError> {
    //     return match self.holon_id {
    //         Some(holon_id) => {
    //             let mut cache_manager_ref_mut = context.cache_manager.borrow_mut();
    //             let holon = cache_manager_ref_mut.get_rc_holon(None, holon_id.clone())?;
    //             Ok(holon)
    //         }
    //         None => {
    //             Err(HolonError::HolonNotFound("No holon_id found in SmartReference".to_string()))
    //         }
    //     }
    //
    // }

    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }
    pub fn get_smart_properties(&self) -> Option<PropertyMap> {
        self.smart_property_values.clone()
    }
    pub fn get_property_map(&self, context: &HolonsContext) -> Result<PropertyMap, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        Ok(holon_refcell.property_map.clone())
    }

    pub fn get_relationship_map(
        &self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        Ok(holon_refcell.relationship_map.clone())
    }

    // Private function for getting a mutable reference from the context
    fn get_rc_holon(&self, context: &HolonsContext) -> Result<Rc<RefCell<Holon>>, HolonError> {
        Ok(context.cache_manager.borrow_mut().get_rc_holon(&self.holon_id)?)
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
            let mut holon_ref = holon.borrow_mut();
            holon_ref.get_related_holons(relationship_name)?.clone()
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
