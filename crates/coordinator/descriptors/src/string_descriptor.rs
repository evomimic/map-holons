use holons::context::HolonsContext;
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapInteger, MapString, ValueType};
use crate::descriptor_types::{Schema, StringDescriptor, TypeDescriptor};
// use shared_types_holon::BaseType::*;

use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};

pub fn define_string_descriptor(
    context: &HolonsContext,
    schema: &Schema,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    min_length: MapInteger,
    max_length: MapInteger,
    has_supertype: Option<&TypeDescriptor>,
    described_by: Option<&TypeDescriptor>,

) -> StringDescriptor {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let mut descriptor = define_type_descriptor(
        context,
        schema,
        derive_descriptor_name(&type_name),
        type_name,
        BaseType::Value(ValueType::String),
        description,
        label,
        MapBoolean(true),
        MapBoolean(true),
        described_by,
        has_supertype,
    );

    descriptor.0
        .with_property_value(
        PropertyName(MapString("min_length".to_string())),
        BaseValue::IntegerValue(min_length),
    )
        .with_property_value(
            PropertyName(MapString("max_length".to_string())),
            BaseValue::IntegerValue(max_length),
        );

    StringDescriptor(descriptor.0)

}