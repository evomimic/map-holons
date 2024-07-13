use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapInteger, MapString, ValueType};

use crate::type_descriptor::{define_type_descriptor, TypeDefinitionHeader};

pub struct IntegerDefinition {
    pub header:TypeDefinitionHeader,
    pub min_value: MapInteger,
    pub max_value: MapInteger,
}

/// This function defines (and describes) a new integer type. Values of this type will be stored
/// as MapInteger. The `min_value` and `max_value` properties are unique to this IntegerType and can
/// be used to narrow the range of legal values for this type. Agent-defined types can be the
/// `ValueType` for a MapProperty.
pub fn define_integer_type(
    context: &HolonsContext,
    schema: &HolonReference,
    definition: IntegerDefinition,
) -> Result<StagedReference, HolonError> {

    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let mut descriptor = define_type_descriptor(
        context,
        schema, // should this be type safe (i.e., pass in either Schema or SchemaTarget)?
        BaseType::Value(ValueType::Integer),
        definition.header,
    )?;

    let mut mut_holon = descriptor.get_mut_holon(context)?;

    mut_holon
        .borrow_mut()
        .with_property_value(
            PropertyName(MapString("min_value".to_string())),
            BaseValue::IntegerValue(definition.min_value),
        )?
        .with_property_value(
            PropertyName(MapString("max_value".to_string())),
            BaseValue::IntegerValue(definition.max_value),
        )?;

    Ok(descriptor)

}