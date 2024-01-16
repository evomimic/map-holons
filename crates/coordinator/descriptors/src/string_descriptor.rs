use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::{BaseType, ValueType};
// use shared_types_holon::BaseType::*;

use crate::type_descriptor::define_type_descriptor;

pub fn define_string_descriptor(
    schema: &RelationshipTarget,
    type_name: String,
    description: String,
    label: String, // Human readable name for this type
    _min_value: i64,
    _max_value: i64,

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let descriptor = define_type_descriptor(
        schema,
        type_name,
        BaseType::Value(ValueType::String),
        description,
        label,
        true,
        true,
    );

    descriptor

}