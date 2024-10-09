use std::collections::BTreeMap;

use shared_types_holon::{PropertyName, PropertyValue, SavedPropertyMap};
use crate::context::HolonsContext;

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
    fn get_property_value_by_id(
        &self,
        property_id: &HolonReference
    ) -> Option<PropertyValue>;

    /// This method assigns a value to the property identified by `property_id`
    /// It does NOT check if `property_id` is a valid property for the owner of this property map.
    /// Such validation checks are the owner's responsibility
    fn with_property_value_by_id(
        &mut self,
        property_id: HolonReference,
        value: PropertyValue
    ) -> &mut Self;
    /// This method is provided for backwards compatibility. It accepts a PropertyName parameter and
    /// does a lookup via the CoreSchema for its DescriptorId and then delegates the call to
    /// `get_property_value_by_id`.
    #[deprecated]
    fn get_property_value(&self,
              _context: &HolonsContext,
              _property_name: &PropertyName
    )
          -> Result<Option<PropertyValue>,HolonError>;

    /// This method is provided for backwards compatibility. It accepts a PropertyName parameter and
    ///  does a lookup via the CoreSchema for its DescriptorId and then delegates the call to
    /// `with_property_value_by_id`.
    #[deprecated]
    fn with_property_value(&mut self,
                           _context: &HolonsContext,
                           _property_name: &PropertyName,
                           _value: PropertyValue
    ) -> Result<&mut Self, HolonError>;
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
    fn get_property_value_by_id(
        &self,
        property_id: &HolonReference
    ) -> Option<PropertyValue> {
        self.get(property_id).cloned()
    }
    fn with_property_value_by_id(
        &mut self,
        property_id: HolonReference,
        value: PropertyValue
    ) -> &mut Self {
        self.insert(property_id, value);
        self
    }

    fn get_property_value(&self,
                          _context: &HolonsContext,
                          _property_name: &PropertyName
    )
        -> Result<Option<PropertyValue>,HolonError> {
        // Implementing this depends on being able to find and query the CoreSchema object
        // let schema = context.get_core_schema();
        // let descriptor_id = schema.get_related_holon_by_key(property_name)?;
        // Ok(get_property_value_by_id(self, descriptor_id))
        todo!()
    }

    fn with_property_value(&mut self,
                           _context: &HolonsContext,
                           _property_name: &PropertyName,
                           _value: PropertyValue
    ) -> Result<&mut Self, HolonError> {
        // Implementing this depends on being able to find and query the CoreSchema object
        // let schema = context.get_core_schema();
        // let descriptor_id = schema.get_related_holon_by_key(property_name)?;
        // Ok(with_property_value_by_id(self, descriptor_id, value))
        todo!()
    }
}

