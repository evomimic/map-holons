use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::{BaseType, MapBoolean, MapInteger, MapString, ValueType};
// use shared_types_holon::BaseType::*;

use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};

pub fn define_string_descriptor(
    schema: &RelationshipTarget,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    _min_value: MapInteger,
    _max_value: MapInteger,

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let descriptor = define_type_descriptor(
        schema,
        derive_descriptor_name(&type_name),
        type_name,
        BaseType::Value(ValueType::String),
        description,
        label,
        MapBoolean(true),
        MapBoolean(true),
    );

    descriptor

}