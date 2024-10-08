use std::collections::BTreeMap;

use shared_types_holon::{PropertyValue, SavedPropertyMap};

use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;

pub type HolonPropertyMap = BTreeMap<HolonReference, PropertyValue>;

pub trait HolonPropertyMapExt {
    /// This function constructs a HolonPropertyMap from a SavedPropertyMap
    fn from_saved_map(saved_map: &SavedPropertyMap) -> Self;

    /// This method constructs a SavedPropertyMap from a HolonPropertyMap
    fn to_saved_map(&self) -> Result<SavedPropertyMap, HolonError>;

    /// This method gets the value (if any) for the property identified by `property_id`
    /// It does NOT check if `property_id` is a valid property for the owner of this property_map.
    /// Such validation checks are the owner's responsibility
    fn get_property_value(
        &self,
        property_id: &HolonReference
    ) -> Option<PropertyValue>;

    /// This method assigns a value to the property identified by `property_id`
    /// It does NOT check if `property_id` is a valid property for the owner of this property map.
    /// Such validation checks are the owner's responsibility
    fn with_property_value(
        &mut self,
        property_id: HolonReference,
        value: PropertyValue
    ) -> &mut Self;
}

impl HolonPropertyMapExt for HolonPropertyMap {
    fn from_saved_map(saved_map: &SavedPropertyMap) -> Self {
        saved_map.iter().map(|(holon_id, property_value)| {
            let holon_reference = HolonReference::from_holon_id(holon_id.clone());
            (holon_reference, property_value.clone())
        }).collect()
    }

    /// This method converts a HolonPropertyMap into a SavedPropertyMap
    fn to_saved_map(&self) -> Result<SavedPropertyMap, HolonError> {
        self.iter().try_fold(BTreeMap::new(), |mut acc,
           (holon_reference, property_value)| {
                match holon_reference {
                    HolonReference::Smart(smart_ref) => {
                        let holon_id = smart_ref.get_holon_id_no_context();
                        acc.insert(holon_id, property_value.clone());
                        Ok(acc)
                    },
                    HolonReference::Staged(_) => {
                        Err(HolonError::InvalidHolonReference("Expected only SmartReferences in the\
                        HolonPropertyMap, but found a StagedReference instead".to_string()))
                    }
                }
           })
    }
    fn get_property_value(
        &self,
        property_id: &HolonReference
    ) -> Option<PropertyValue> {
        self.get(property_id).cloned()
    }
    fn with_property_value(
        &mut self,
        property_id: HolonReference,
        value: PropertyValue
    ) -> &mut Self {
        self.insert(property_id, value);
        self
    }
}
