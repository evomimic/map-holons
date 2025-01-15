// This file defines the TypeDescriptor struct and the dance functions it supports

use crate::descriptor_types::CoreSchemaRelationshipTypeName::{DescribedBy, OwnedBy};
use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName};
use crate::semantic_version::SemanticVersion;
use hdk::prelude::{debug, info};
use holons::reference_layer::{
    HolonReference, HolonWritable, HolonsContextBehavior, StagedReference,
};
use holons::{Holon, HolonError};
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapEnumValue, MapString};
use CoreSchemaPropertyTypeName::*;

#[derive(Debug, Clone)]
pub struct TypeDescriptorDefinition {
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub is_dependent: MapBoolean,
    pub is_value_type: MapBoolean,
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub is_subtype_of: Option<HolonReference>, // Type-IS_SUBTYPE_OF->Type
    pub owned_by: Option<HolonReference>,     // Holon-OwnedBy->HolonSpace
                                              // pub key_properties: Option<Vec<HolonReference>>,
                                              //pub descriptor_properties: Vec<HolonReference>, // Type-DESCRIPTOR_PROPERTIES->PropertyType
                                              //pub descriptor_relationships: Vec<HolonReference>, // Type-DESCRIPTOR_RELATIONSHIPS->RelationshipType
}

/// This is a helper function that defines and stages (but does not commit) a new TypeDescriptor.
/// It is intended to be called by other define_xxx_descriptor functions.
///
/// This function adds values for each of the properties shared by all type descriptors
/// and (optionally) adds related holons for relationships shared by all type descriptors
///
/// For now, `version` is being treated as a MapString property and is initialized to "0.0.1"
///
/// This function will add the `Type-COMPONENT_OF->Schema` relationship
/// and optionally, the following relationships:
/// * `Type-DESCRIBED_BY->TypeDescriptor` (if supplied)
/// * `Holon-OwnedBy-> HolonSpace` (if supplied)
/// * `Type-HAS_SUPERTYPE->TypeDescriptor` (if supplied)
///
///
pub fn define_type_descriptor(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference, // Type-COMPONENT_OF->Schema
    base_type: BaseType,
    definition: TypeDescriptorDefinition,
) -> Result<StagedReference, HolonError> {
    info!("Staging... {:#?}", definition.descriptor_name.clone());

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // Define a default semantic_version as a String Property
    let initial_version = MapString(SemanticVersion::default().to_string());

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(definition.descriptor_name.clone()),
        )?
        .with_property_value(
            DescriptorName.as_property_name(),
            BaseValue::StringValue(definition.descriptor_name.clone()),
        )?
        .with_property_value(
            Description.as_property_name(),
            BaseValue::StringValue(definition.description),
        )?
        .with_property_value(Label.as_property_name(), BaseValue::StringValue(definition.label))?
        .with_property_value(
            CoreSchemaPropertyTypeName::BaseType.as_property_name(),
            BaseValue::EnumValue(MapEnumValue(MapString(base_type.to_string()))),
        )?
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(definition.is_dependent),
        )?
        .with_property_value(
            PropertyName(MapString("is_value_descriptor".to_string())),
            BaseValue::BooleanValue(definition.is_value_type),
        )?
        .with_property_value(
            PropertyName(MapString("version".to_string())),
            BaseValue::StringValue(initial_version),
        )?;

    // Stage the new TypeDescriptor

    debug!("{:#?}", descriptor.clone());

    let staged_reference = context.get_space_manager().stage_new_holon(descriptor.clone())?;

    // Add related holons

    staged_reference.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::ComponentOf.as_rel_name(),
        vec![schema.clone()],
    )?;

    if let Some(descriptor_ref) = definition.described_by {
        staged_reference.add_related_holons(
            context,
            DescribedBy.as_rel_name(),
            vec![descriptor_ref],
        )?
    };
    if let Some(is_subtype_of_ref) = definition.is_subtype_of {
        staged_reference.add_related_holons(
            context,
            CoreSchemaRelationshipTypeName::IsA.as_rel_name(),
            vec![is_subtype_of_ref],
        )?
    };
    if let Some(owned_by_ref) = definition.owned_by {
        staged_reference.add_related_holons(context, OwnedBy.as_rel_name(), vec![owned_by_ref])?
    };

    // if header.descriptor_properties.len()>0 {
    //     staged_reference
    //         .add_related_holons(
    //             context,
    //             RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
    //             header.descriptor_properties)?
    // };
    //
    // if header.descriptor_relationships.len()>0 {
    //     staged_reference
    //         .add_related_holons(
    //             context,
    //             RelationshipName(MapString("DESCRIPTOR_RELATIONSHIPS".to_string())),
    //             header.descriptor_relationships)?
    // };

    Ok(staged_reference)
}
