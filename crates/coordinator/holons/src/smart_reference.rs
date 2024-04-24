use std::rc::Rc;

use derive_new::new;
use hdk::prelude::*;

use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{HolonId, MapString, PropertyMap, PropertyValue};

use crate::context::HolonsContext;
use crate::holon::{Holon, HolonFieldGettable};
use crate::holon_errors::HolonError;
use crate::relationship::RelationshipMap;
// use crate::staged_reference::StagedReference;
use crate::holon::HolonState;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartReference {
    //holon_space_id: Option<SpaceId>
    pub holon_id: HolonId,
    pub key: Option<MapString>,
    pub rc_holon: Option<Rc<Holon>>,
    pub smart_property_values: Option<PropertyMap>,
}

impl SmartReference {
    /// This is a private function that attempts to ensure that the SmartReference contains a populated rc_holon field.
    /// If rc_holon is already populated, it simply returns Ok(())
    /// Otherwise, invoke get_rc_holon on the cache_manager (found in the context) to get a reference to the cached holon,

    fn ensure_rc(&mut self, context: &HolonsContext) -> Result<(), HolonError> {
        // Check if rc_holon is already populated
        if self.rc_holon.is_some() {
            return Ok(()); // Already populated, no action needed
        }

        // Obtain a mutable reference to cache_manager
        let mut cache_manager_ref_mut = context.cache_manager.borrow_mut();

        // Attempt to populate rc_holon by invoking get_rc_holon on the cache_manager
        let rc_holon = cache_manager_ref_mut.get_rc_holon(context, None, &self.holon_id)?;

        // Update rc_holon in self
        self.rc_holon = Some(rc_holon);

        Ok(()) // rc_holon has been ensured to be populated
    }
    // fn ensure_rc(&mut self, context: &HolonsContext,)->Result<(), HolonError> {
    //     // Check if rc_holon is already populated
    //     if self.rc_holon.is_some() {
    //         return Ok(()); // Already populated, no action needed
    //     }
    //
    //     // TODO: Attempt to populate rc_holon by invoking get_rc_holon on the cache_manager
    //     let rc_holon = context.cache_manager.borrow().get_rc_holon(context, None, &self.holon_id)?;
    //     self.rc_holon = Some(rc_holon);
    //
    //     Ok(()) // rc_holon has been ensured to be populated
    //
    // }

    // // Should only be called by the CommitManager ie TODO: needs to be protectecd
    // pub fn clone_holon(&mut self, context: &HolonsContext) -> Result<Holon, HolonError> {
    //     // Ensure rc_holon is populated
    //     self.ensure_rc(context)?;

    //     // Call the method directly on the dereferenced Rc
    //     if let Some(rc_holon) = self.rc_holon.as_ref() {
    //         let holon_key = rc_holon.get_key()?;
    //         let mut holon = Holon::new();

    //         holon.property_map = rc_holon.property_map.clone();
    //         holon.relationship_map = rc_holon.relationship_map.clone();

    //         if let Some(key) = holon_key {
    //             holon.set_key_manually(key);
    //         } else {
    //         }

    //         Ok(holon)
    //     } else {
    //         Err(HolonError::InvalidHolonReference(
    //             "Rc Holon is not available".to_string(),
    //         ))
    //     }
    // }

    pub fn clone_reference(&self) -> SmartReference {
        SmartReference {
            holon_id: self.holon_id.clone(),
            key: self.key.clone(),
            rc_holon: self.rc_holon.clone(),
            smart_property_values: self.smart_property_values.clone(),
        }
    }

    // Constructor function for creating from Holon Reference
    pub fn from_holon(rc_holon: Rc<Holon>) -> Result<SmartReference, HolonError> {
        let id = rc_holon.get_id()?;
        let key = rc_holon.get_key()?;

        Ok(SmartReference {
            holon_id: id,
            key,
            rc_holon: Some(rc_holon),
            smart_property_values: None, // TODO: need fn to build smart_property_map, this requires descriptor
        })
    }
    pub fn get_property_map(&mut self, context: &HolonsContext) -> Result<PropertyMap, HolonError> {
        // Ensure rc_holon is populated
        self.ensure_rc(context)?;

        // Call the method directly on the dereferenced Rc
        if let Some(holon) = self.rc_holon.as_ref() {
            Ok(holon.property_map.clone())
        } else {
            Err(HolonError::InvalidHolonReference(
                "Rc Holon is not available".to_string(),
            ))
        }
    }

    pub fn get_relationship_map(
        &mut self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        // Ensure rc_holon is populated
        self.ensure_rc(context)?;

        // Call the method directly on the dereferenced Rc
        if let Some(holon) = self.rc_holon.as_ref() {
            Ok(holon.relationship_map.clone())
        } else {
            Err(HolonError::InvalidHolonReference(
                "Rc Holon is not available".to_string(),
            ))
        }
    }
}
impl HolonFieldGettable for SmartReference {
    fn get_property_value(
        &mut self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        // Check if the property value is available in smart_property_values
        if let Some(smart_map) = &self.smart_property_values {
            if let Some(value) = smart_map.get(property_name) {
                return Ok(value.clone());
            }
        }

        // Ensure rc_holon is populated
        self.ensure_rc(context)?;

        // Call the method directly on the dereferenced Rc
        if let Some(holon) = self.rc_holon.as_ref() {
            holon.get_property_value(property_name)
        } else {
            Err(HolonError::InvalidHolonReference(
                "Rc Holon is not available".to_string(),
            ))
        }
    }

    fn get_key(&mut self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        // Ensure rc_holon is populated
        self.ensure_rc(context)?;

        // Call the method directly on the dereferenced Rc
        if let Some(holon) = self.rc_holon.as_ref() {
            holon.get_key()
        } else {
            Err(HolonError::InvalidHolonReference(
                "Rc Holon is not available".to_string(),
            ))
        }
    }
}
