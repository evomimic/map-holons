use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::{BaseType, MapBoolean, MapInteger, MapString, ValueType};
use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};

pub fn define_integer_descriptor(
    schema: &RelationshipTarget,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    _min_length: MapInteger,
    _max_length: MapInteger,

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let descriptor = define_type_descriptor(
        schema, // should this be type safe (i.e., pass in either Schema or SchemaTarget)?
        derive_descriptor_name(&type_name),
        type_name,
        BaseType::Value(ValueType::Integer),
        description,
        label,
        MapBoolean(true),
        MapBoolean(true),
    );

    // TODO: Create PropertyDescriptors for min_length & max_length
    // TODO: get the (assumed to be existing HAS_PROPERTIES RelationshipDescriptor)
    // TODO: add the property descriptors to the TypeDescriptors HAS_PROPERTIES relationship



    descriptor

}