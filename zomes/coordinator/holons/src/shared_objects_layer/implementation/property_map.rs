use shared_types_holon::holon_node::{PropertyMap, PropertyName};
// use crate::shared_objects_layer::api::
//
// use crate::context::HolonsContext;
use crate::reference_layer::HolonsContextBehavior;
use crate::shared_objects_layer::HolonError;
use shared_types_holon::PropertyValue;

pub fn get_property_value(
    property_map: PropertyMap,
    _context: &dyn HolonsContextBehavior,
    property_name: &PropertyName,
) -> Result<PropertyValue, HolonError> {
    property_map
        .get(property_name)
        .cloned()
        .ok_or_else(|| HolonError::EmptyField(property_name.to_string()))
}
