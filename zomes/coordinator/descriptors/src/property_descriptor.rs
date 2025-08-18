use hdi::prelude::debug;

use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName};
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};

use holons_core::{core_shared_objects::{stage_new_holon_api, TransientHolon}, HolonReference, HolonsContextBehavior, StagedReference, WriteableHolon};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, TypeKind};
use integrity_core_types::PropertyName;

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
        define_type_descriptor(context, schema, TypeKind::Property, definition.header)?;

    // Build the PropertyType
    let mut property_type = TransientHolon::new();

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
        CoreSchemaRelationshipTypeName::TypeDescriptor,
        vec![HolonReference::Staged(type_descriptor_ref)],
    )?;
    property_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::ValueType,
        vec![definition.value_type.clone()],
    )?;

    Ok(property_type_ref)
}
