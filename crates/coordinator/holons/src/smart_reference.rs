use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use derive_new::new;
use hdk::prelude::*;

use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{HolonId, MapString, PropertyMap, PropertyValue};

use crate::context::HolonsContext;
use crate::holon::Holon;
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::holon_property_map::HolonPropertyMap;
use crate::holon_reference::{HolonGettable, HolonReference};
use crate::relationship::{RelationshipMap, RelationshipName};

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartReference {
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl PartialOrd for SmartReference {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.holon_id.partial_cmp(&other.holon_id)
    }
}

impl Ord for SmartReference {
    fn cmp(&self, other: &Self) -> Ordering {
        self.holon_id.cmp(&other.holon_id)
    }
}

impl SmartReference {
    /// Constructor for SmartReference that takes a HolonId and sets smart_property_values to None
    pub fn new_from_id(holon_id: HolonId) -> Self {
        SmartReference {
            holon_id,
            smart_property_values: None,
        }
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
    /// This method allows the HolonId for a SmartReference to be retrieved without having to
    /// provide a context
    pub(crate) fn get_holon_id_no_context(&self) -> HolonId {
        self.holon_id.clone()
    }


   pub fn get_holon_property_map(&self, context: &HolonsContext) -> Result<HolonPropertyMap, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        Ok(holon_refcell.get_holon_property_map()?.clone())
    }

    /// This method is provided for backwards compatibility. It accepts a PropertyName parameter and
    /// does a lookup via this holon's HolonDescriptor to get a HolonReference to the property's
    /// PropertyDescriptor and then delegates the call to `get_property_value_by_descriptor`.
    pub fn get_property_value(
        &self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        let holon_ref = HolonReference::Smart(self.clone());
        let descriptor_reference = holon_ref
            .get_property_descriptor_by_name(context, property_name)?;
        self.get_property_value_by_descriptor(context, &descriptor_reference)

    }

    pub fn get_relationship_map(
        &self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        Ok(holon_refcell.relationship_map.clone())
    }

    // Private function for getting direct access to the referenced holon
    fn get_rc_holon(&self, context: &HolonsContext) -> Result<Rc<RefCell<Holon>>, HolonError> {
        Ok(context
            .cache_manager
            .borrow_mut()
            .get_rc_holon(context, &self.holon_id)?)
    }
    pub fn get_smart_properties(&self) -> Option<PropertyMap> {
        self.smart_property_values.clone()
    }
}
impl HolonGettable for SmartReference {

    fn get_holon_id(&self, _context: &HolonsContext) -> Result<Option<HolonId>, HolonError> {
        Ok(Some(self.holon_id.clone()))
    }

    /// This function gets the value for the specified property descriptor
    /// It will attempt to get it from the smart_property_values map first to avoid having to
    /// retrieve the underlying holon. But, failing that, it will get its referenced holon from
    /// the cache and delegate the call to its referenced holon.
    ///
    /// Possible Errors:
    /// This function returns an EmptyField error if no value is found for the specified property
    /// Or (less likely) an InvalidHolonReference
    fn get_property_value_by_descriptor(
        &self,
        context: &HolonsContext,
        property_descriptor: &HolonReference,
    ) -> Result<PropertyValue, HolonError> {
        // TODO: Issue #164 -- uncomment the following
        // Check if the property value is available in smart_property_values
        // if let Some(smart_map) = &self.smart_property_values {
        //     if let Some(value) = smart_map.get(property_descriptor) {
        //         return Ok(value.clone());
        //     }
        // }

        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        holon_refcell.get_property_value_by_descriptor(property_descriptor).clone()
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
                let key = holon.borrow_mut().get_key(context)?;
                Ok(key)
            }
        } else {
            let holon = self.get_rc_holon(context)?;
            let key = holon.borrow_mut().get_key(context)?;
            Ok(key)
        }
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
            holon_ref.get_related_holons(context, relationship_name)?.clone()
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
