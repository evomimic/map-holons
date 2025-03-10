use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName};
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};
use hdi::prelude::debug;
use holons_core::core_shared_objects::{Holon, HolonError};
use holons_core::holon_operations_api::*;
use holons_core::{HolonReference, HolonWritable, HolonsContextBehavior, StagedReference};
use shared_types_holon::{BaseType, BaseValue, MapString, PropertyName};

pub struct PropertyTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub property_name: PropertyName,
    pub value_type: HolonReference, // should be reference to the ValueType for this property
}
/// This function defines and stages (but does not persist) a new PropertyDescriptor.
/// Values for each of the PropertyDescriptor properties will be set based on supplied parameters.
///
/// *Naming Rules:*
/// * `type_name` will be automatically derived based on the following rule:
///    `
/// *  `descriptor_name` will be derived by appending `Descriptor` to its type_name
///
/// The descriptor will have the following relationships populated:
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * PROPERTY_OF->HolonDescriptor (supplied)
/// * VALUE_TYPE->ValueDescriptor (supplied)
///
///
pub fn define_property_type(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    definition: PropertyTypeDefinition,
) -> Result<StagedReference, HolonError> {
    let type_descriptor_ref =
        define_type_descriptor(context, schema, BaseType::Property, definition.header)?;

    // Build the PropertyType
    let mut property_type = Holon::new();

    // Add properties

    property_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(definition.property_name.0.clone()),
        )?
        .with_property_value(
            CoreSchemaPropertyTypeName::PropertyTypeName.as_property_name(),
            BaseValue::StringValue(definition.property_name.0.clone()),
        )?;

    // Stage the PropertyType

    debug!("Staging... {:#?}", property_type.clone());

    let property_type_ref = stage_new_holon_api(context, property_type.clone())?;

    // Populate the relationships

    property_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(type_descriptor_ref)],
    )?;
    property_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::ValueType.as_rel_name(),
        vec![definition.value_type.clone()],
    )?;

    Ok(property_type_ref)
}
