use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::{BaseType, ValueType};
use crate::type_descriptor::define_type_descriptor;

pub fn define_integer_descriptor(
    schema: &RelationshipTarget,
    type_name: String,
    description: String,
    label: String, // Human readable name for this type
    _min_length: i64,
    _max_length: i64,

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let descriptor = define_type_descriptor(
        schema, // should this be type safe (i.e., pass in either Schema or SchemaTarget)?
        type_name,
        BaseType::Value(ValueType::Integer),
        description,
        label,
        true,
        true,
    );

    // TODO: Create PropertyDescriptors for min_length & max_length
    // TODO: get the (assumed to be existing HAS_PROPERTIES RelationshipDescriptor)
    // TODO: add the property descriptors to the TypeDescriptors HAS_PROPERTIES relationship



    descriptor

}