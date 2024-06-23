use std::rc::Rc;

use derive_new::new;
use hdk::prelude::*;

use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{HolonId, MapString, PropertyMap, PropertyValue};

use crate::context::HolonsContext;
use crate::holon::{Holon, HolonGettable};
use crate::holon_error::HolonError;
use crate::relationship::RelationshipMap;
use crate::smartlink::decode_link_tag;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartReference {
    //holon_space_id: Option<SpaceId>
    pub holon_id: HolonId,
    pub smart_property_values: Option<PropertyMap>,
}

impl SmartReference {
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

    // Constructor function for creating Holon Reference from an rc_holon
    pub fn from_holon(rc_holon: Rc<Holon>) -> Result<SmartReference, HolonError> {
        let id = rc_holon.get_id()?;

        Ok(SmartReference {
            holon_id: id,
            smart_property_values: None, // TODO: need fn to build smart_property_map, this requires descriptor
        })
    }
    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }
    pub fn get_property_map(&self, context: &HolonsContext) -> Result<PropertyMap, HolonError> {
        if let Ok(holon) = context
            .cache_manager
            .borrow_mut()
            .get_rc_holon(None, &self.holon_id)
        {
            let holon_refcell = holon.borrow();
            Ok(holon_refcell.property_map.clone())
        } else {
            Err(HolonError::InvalidHolonReference(
                "Rc Holon is not available".to_string(),
            ))
        }
    }
    // temporary function used until descriptors are ready
    pub fn get_key_from_link(&self, link: Link) -> Result<Option<MapString>, HolonError> {
        let link_tag_bytes = link.tag.clone().into_inner();
        let link_tag = String::from_utf8(link_tag_bytes).map_err(|_e| {
            HolonError::Utf8Conversion(
                "Link tag bytes".to_string(),
                "String (relationship name)".to_string(),
            )
        })?;
        let decoded_prop_map = decode_link_tag(link_tag).smart_property_values;
        if let Some(prop_vals) = decoded_prop_map {
            let option_key = prop_vals.get(&PropertyName(MapString("key".to_string())));
            if let Some(key) = option_key {
                return Ok(Some(MapString(key.into())));
            } else {
                return Ok(None);
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_relationship_map(
        &self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        if let Ok(holon) = context
            .cache_manager
            .borrow_mut()
            .get_rc_holon(None, &self.holon_id)
        {
            let holon_refcell = holon.borrow();
            Ok(holon_refcell.relationship_map.clone())
        } else {
            Err(HolonError::InvalidHolonReference(
                "Rc Holon is not available".to_string(),
            ))
        }
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
        if let Ok(holon) = context
            .cache_manager
            .borrow_mut()
            .get_rc_holon(None, &self.holon_id)
        {
            holon.borrow().get_property_value(property_name)
        } else {
            Err(HolonError::InvalidHolonReference(
                "Rc Holon is not available".to_string(),
            ))
        }
    }

    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        if let Ok(holon) = context
            .cache_manager
            .borrow_mut()
            .get_rc_holon(None, &self.holon_id)
        {
            holon.borrow().get_key()
        } else {
            Err(HolonError::InvalidHolonReference(
                "Rc Holon is not available".to_string(),
            ))
        }
    }
}
