use holons::holon_reference::HolonReference;
use holons::holon_types::{Holon};
use holons::relationship::{RelationshipName, RelationshipTarget};
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapInteger, MapString, ValueType};
use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};

pub fn define_integer_descriptor(
    schema: &RelationshipTarget,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    min_value: MapInteger,
    max_value: MapInteger,
    has_supertype: Option<HolonReference>,
    described_by: Option<HolonReference>,

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let mut descriptor = define_type_descriptor(
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

    descriptor
        .with_property_value(
            PropertyName(MapString("min_value".to_string())),
            BaseValue::IntegerValue(min_value),
        )
        .with_property_value(
            PropertyName(MapString("max_value".to_string())),
            BaseValue::IntegerValue(max_value),
        );

    // Populate the relationships

    descriptor
        .add_related_holon(
            RelationshipName(MapString("COMPONENT_OF".to_string())),
            schema.clone(),
        );


    // TODO: Create PropertyDescriptors for min_length & max_length
    // TODO: get the (assumed to be existing HAS_PROPERTIES RelationshipDescriptor)
    // TODO: add the property descriptors to the TypeDescriptors HAS_PROPERTIES relationship



    descriptor

}