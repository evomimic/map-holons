use holons::context::HolonsContext;
use holons::holon_reference::HolonReference;


use holons::staged_reference::StagedReference;
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapInteger, MapString, ValueType};
use crate::descriptor_types::{IntegerType};
use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};
/// This function defines (and describes) a new integer type. Values of this type will be stored
/// as MapInteger. The `min_value` and `max_value` properties are unique to this IntegerType and can
/// be used to narrow the range of legal values for this type. Agent-defined types can be the
/// `ValueType` for a MapProperty.
pub fn define_integer_type(
    context: &HolonsContext,
    schema: HolonReference,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    min_value: MapInteger,
    max_value: MapInteger,
    has_supertype: Option<StagedReference>, // this should always be ValueType
    described_by: Option<StagedReference>,

) -> IntegerType {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let mut descriptor = define_type_descriptor(
        context,
        schema, // should this be type safe (i.e., pass in either Schema or SchemaTarget)?
        derive_descriptor_name(&type_name),
        type_name,
        BaseType::Value(ValueType::Integer),
        description,
        label,
        MapBoolean(true),
        MapBoolean(true),
        described_by,
        has_supertype,
    );

    descriptor.0
        .with_property_value(
            PropertyName(MapString("min_value".to_string())),
            BaseValue::IntegerValue(min_value),
        )
        .with_property_value(
            PropertyName(MapString("max_value".to_string())),
            BaseValue::IntegerValue(max_value),
        );

    IntegerType(descriptor.0)

}