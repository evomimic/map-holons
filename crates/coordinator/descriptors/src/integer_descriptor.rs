use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName};
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::value_types::{BaseType, BaseValue, MapInteger, MapString, ValueType};
use shared_types_holon::PropertyName;

use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};

#[derive(Clone)]
pub struct IntegerTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub type_name: MapString,
    pub min_value: MapInteger,
    pub max_value: MapInteger,
}

/// This function defines (and describes) a new integer type. Values of this type will be stored
/// as MapInteger. The `min_value` and `max_value` properties are unique to this IntegerType and can
/// be used to narrow the range of legal values for this type. Agent-defined types can be the
/// `ValueType` for a MapProperty.
pub fn define_integer_type(
    context: &HolonsContext,
    schema: &HolonReference,
    definition: IntegerTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let type_descriptor_ref = define_type_descriptor(
        context,
        schema, // should this be type safe (i.e., pass in either Schema or SchemaTarget)?
        BaseType::Value(ValueType::Integer),
        definition.header.clone(),
    )?;

    let mut integer_type = Holon::new();

    // Add its properties

    integer_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(definition.type_name.clone()),
        )?
        .with_property_value(
            PropertyName(MapString(
                CoreSchemaPropertyTypeName::TypeName.as_snake_case().to_string(),
            )),
            BaseValue::StringValue(definition.type_name.clone()),
        )?
        .with_property_value(
            CoreSchemaPropertyTypeName::MinValue.as_property_name(),
            BaseValue::IntegerValue(definition.min_value),
        )?
        .with_property_value(
            CoreSchemaPropertyTypeName::MaxValue.as_property_name(),
            BaseValue::IntegerValue(definition.max_value),
        )?;

    // Stage new holon type
    let integer_type_ref =
        context.commit_manager.borrow_mut().stage_new_holon(integer_type.clone())?;

    // Add some relationships

    integer_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(type_descriptor_ref)],
    )?;

    Ok(integer_type_ref)
}
