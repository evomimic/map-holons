use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{
    BaseType, BaseValue, MapBoolean, MapInteger, MapString, ValueType,
};

use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};

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
    type_name: MapString,
    description: MapString,
    label: MapString,
    has_supertype: Option<HolonReference>,
    described_by: Option<HolonReference>,
    owned_by: Option<HolonReference>,
    min_length: MapInteger,
    max_length: MapInteger,
) -> Result<StagedReference, HolonError> {

    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let mut descriptor = define_type_descriptor(
        context,
        schema,
        derive_descriptor_name(&type_name),
        type_name,
        BaseType::Value(ValueType::String),
        description,
        label,
        MapBoolean(true), // is_dependent
        MapBoolean(true), // is_value_type
        described_by,
        has_supertype,
        owned_by,
    )?;

    let mut mut_holon = descriptor.get_mut_holon(context)?;

    mut_holon
        .borrow_mut()
        .with_property_value(
            PropertyName(MapString("min_length".to_string())),
            BaseValue::IntegerValue(min_length),
        )?
        .with_property_value(
            PropertyName(MapString("max_length".to_string())),
            BaseValue::IntegerValue(max_length),
        )?;


    Ok(descriptor)
}
