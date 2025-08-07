use hdi::prelude::debug;

use crate::descriptor_types::{
    CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName,
    CoreSchemaRelationshipTypeName::KeyProperties,
};
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, TypeKind};
use holons_core::{
    core_shared_objects::{stage_new_holon_api, TransientHolon},
    HolonReference, HolonsContextBehavior, StagedReference, WriteableHolon,
};
use integrity_core_types::PropertyName;

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
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    definition: HolonTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------

    let type_descriptor_ref =
        define_type_descriptor(context, schema, TypeKind::Holon, definition.header.clone())?;

    // Build new HolonType

    let mut holon_type = TransientHolon::new();

    // Add its properties
    holon_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(definition.type_name.clone())),
        )?
        .with_property_value(
            PropertyName(MapString(
                CoreSchemaPropertyTypeName::TypeName.as_snake_case().to_string(),
            )),
            Some(BaseValue::StringValue(definition.type_name.clone())),
        )?;

    debug!("Staging new holon_type {:#?}", holon_type.clone());

    // Stage new holon type
    let holon_type_ref = stage_new_holon_api(context, holon_type.clone())?;

    // Add some relationships

    holon_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor,
        vec![HolonReference::Staged(type_descriptor_ref)],
    )?;

    if definition.properties.len() > 0 {
        holon_type_ref.add_related_holons(
            context,
            CoreSchemaRelationshipTypeName::Properties,
            definition.properties,
        )?;
    }

    if let Some(key_properties) = definition.key_properties {
        holon_type_ref.add_related_holons(
            context,
            KeyProperties,
            key_properties,
        )?
    };

    Ok(holon_type_ref)
}
