use holons::context::HolonsContext;
use holons::holon_reference::HolonReference;

use holons::staged_reference::StagedReference;
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapInteger, MapString, ValueType};
use crate::descriptor_types::{StringType};
// use shared_types_holon::BaseType::*;

use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};
use crate::value_type_descriptor::define_value_type;

pub fn define_string_type(
    context: &HolonsContext,
    schema: &HolonReference,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    min_length: MapInteger,
    max_length: MapInteger,
    has_supertype: Option<StagedReference>,
    described_by: Option<StagedReference>,

) -> StringType {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let mut descriptor = define_value_type(
        context,
        schema,
        derive_descriptor_name(&type_name),
        type_name,
        BaseType::Value(ValueType::String),
        description,
        label,
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

    StringType(descriptor.0)

}