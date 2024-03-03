
use crate::holon_errors::HolonError;

use shared_types_holon::holon_node::{PropertyMap, PropertyName};

use shared_types_holon::{PropertyValue};
use crate::context::HolonsContext;


pub fn get_property_value(property_map: PropertyMap, _context: &HolonsContext, property_name: &PropertyName) -> Result<PropertyValue, HolonError> {
        property_map.get(property_name).cloned().ok_or_else(|| HolonError::EmptyField(property_name.to_string()))
    }






