use holons::context::HolonsContext;
use holons::holon_reference::HolonReference;

use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapInteger, MapString, ValueType};
use crate::descriptor_types::{IntegerDescriptor, Schema, TypeDescriptor};
use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};

pub fn define_integer_descriptor(
    context: &HolonsContext,
    schema: StagedReference,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    min_value: MapInteger,
    max_value: MapInteger,
    has_supertype: Option<StagedReference>,
    described_by: Option<StagedReference>,

) -> IntegerDescriptor {
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

    // Populate the relationships

    // descriptor.0
    //     .add_related_holon(
    //         RelationshipName(MapString("COMPONENT_OF".to_string())),
    //         define_local_target(&schema.0.clone()),
    //     );
    //

    // TODO: Create PropertyDescriptors for min_length & max_length
    // TODO: get the (assumed to be existing HAS_PROPERTIES RelationshipDescriptor)
    // TODO: add the property descriptors to the TypeDescriptors HAS_PROPERTIES relationship



    IntegerDescriptor(descriptor.0)

}