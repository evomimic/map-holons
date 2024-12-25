use crate::descriptor_types::CoreSchemaRelationshipTypeName::KeyProperties;
use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName};
use hdi::prelude::debug;
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::holon_writable::HolonWritable;
use holons::space_manager::HolonStagingBehavior;
use holons::staged_reference::StagedReference;
use shared_types_holon::value_types::MapString;
use shared_types_holon::{BaseType, BaseValue, PropertyName};

use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};

#[derive(Clone, Debug)]
pub struct HolonTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub type_name: MapString,
    pub properties: Vec<HolonReference>, // Property Descriptors
    pub key_properties: Option<Vec<HolonReference>>, // Order list of property names comprising the key for this holon type
                                                     // pub source_for: Vec<HolonReference>, // Relationship Descriptors will be added after HolonDefinition is staged?
                                                     // pub dances: Vec<HolonReference>, // DanceDescriptors
}

/// This function defines and stages (but does not persist) a new HolonType.
/// It adds values for each of its properties based on supplied parameters
/// and (optionally) it adds related holons for this type's relationships
///
/// *Naming Rule*:
///     `descriptor_name`:= `<type_name>"HolonDescriptor"`
///
/// The descriptor will have the following relationships populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonDescriptor (if supplied)
/// * OWNED_BY->HolonSpace (if supplied)
/// * PROPERTIES->PropertyDescriptor (if supplied)
/// * SOURCE_FOR->RelationshipDescriptor (if supplied)
///
pub fn define_holon_type(
    context: &HolonsContext,
    schema: &HolonReference,
    definition: HolonTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------

    let type_descriptor_ref =
        define_type_descriptor(context, schema, BaseType::Holon, definition.header.clone())?;

    // Build new HolonType

    let mut holon_type = Holon::new();

    // Add its properties
    holon_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(definition.type_name.clone()),
        )?
        .with_property_value(
            PropertyName(MapString(
                CoreSchemaPropertyTypeName::TypeName.as_snake_case().to_string(),
            )),
            BaseValue::StringValue(definition.type_name.clone()),
        )?;

    debug!("Staging new holon_type {:#?}", holon_type.clone());

    // Stage new holon type
    let holon_type_ref = context.space_manager.borrow().stage_new_holon(holon_type.clone())?;

    // Add some relationships

    holon_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(type_descriptor_ref)],
    )?;

    if definition.properties.len() > 0 {
        holon_type_ref.add_related_holons(
            context,
            CoreSchemaRelationshipTypeName::Properties.as_rel_name(),
            definition.properties,
        )?;
    }

    if let Some(key_properties) = definition.key_properties {
        holon_type_ref.add_related_holons(context, KeyProperties.as_rel_name(), key_properties)?
    };

    Ok(holon_type_ref)
}
