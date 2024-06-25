use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{
    BaseType, BaseValue, MapInteger, MapString, ValueType,
};

use crate::type_descriptor::{define_type_descriptor, TypeDefinitionHeader};

pub struct StringDefinition {
    pub header:TypeDefinitionHeader,
    pub min_length: MapInteger,
    pub max_length: MapInteger,
}

/// This function defines and stages (but does not persist) a new StringValueType
/// Values for each of its properties will be set based on supplied parameters.
///
/// *Naming Rule*:
///     `descriptor_name`:= `<type_name>"ValueDescriptor"`
///
/// The descriptor will have the following relationships populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonDescriptor (if supplied)
///
pub fn define_string_type(
    context: &HolonsContext,
    schema: &HolonReference,
    definition: StringDefinition,
) -> Result<StagedReference, HolonError> {

    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let descriptor = define_type_descriptor(
        context,
        schema,
        BaseType::Value(ValueType::String),
        definition.header,
    )?;

    let mut mut_holon = descriptor.get_mut_holon(context)?;

    mut_holon
        .borrow_mut()
        .with_property_value(
            PropertyName(MapString("min_length".to_string())),
            BaseValue::IntegerValue(definition.min_length),
        )?
        .with_property_value(
            PropertyName(MapString("max_length".to_string())),
            BaseValue::IntegerValue(definition.max_length),
        )?;


    Ok(descriptor)
}
