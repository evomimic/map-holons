// This file defines the TypeDescriptor struct and the dance functions it supports

use hdk::prelude::{info,debug,warn};
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapEnumValue, MapString};

use crate::semantic_version::SemanticVersion;

pub struct TypeDefinitionHeader {
    pub descriptor_name: Option<MapString>,  // If None, the descriptor name will be derived from the type_name
    pub type_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub is_dependent: MapBoolean,
    pub is_value_type: MapBoolean,
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub is_subtype_of: Option<HolonReference>, // Type-IS_SUBTYPE_OF->Type
    pub owned_by: Option<HolonReference>, // Holon-OWNED_BY->HolonSpace
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
/// * `Holon-OWNED_BY-> HolonSpace` (if supplied)
/// * `Type-HAS_SUPERTYPE->TypeDescriptor` (if supplied)
///
///
pub fn define_type_descriptor(
    context: &HolonsContext,
    schema: &HolonReference, // Type-COMPONENT_OF->Schema
    base_type: BaseType,
    header: TypeDefinitionHeader,
) -> Result<StagedReference, HolonError> {

    info!("Staging... {:#?}", header.type_name.clone());

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    // Define a default semantic_version as a String Property
    let initial_version = MapString(SemanticVersion::default().to_string());
    let descriptor_name = match header.descriptor_name {
        Some(supplied_name)=> supplied_name,
        None=> derive_descriptor_name(&header.type_name.clone())
    };

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(header.type_name.clone()),
        )?
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(header.type_name),
        )?
        .with_property_value(
            PropertyName(MapString("descriptor_name".to_string())),
            BaseValue::StringValue(descriptor_name),
        )?
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(header.description),
        )?
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(header.label),
        )?
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::EnumValue(MapEnumValue(MapString(base_type.to_string()))),
        )?
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(header.is_dependent),
        )?
        .with_property_value(
            PropertyName(MapString("is_value_descriptor".to_string())),
            BaseValue::BooleanValue(header.is_value_type),

        )?
        .with_property_value(
            PropertyName(MapString("version".to_string())),
            BaseValue::StringValue(initial_version),
        )?;

    // Stage the new TypeDescriptor

    debug!("{:#?}", descriptor.clone());

    let staged_reference = context
        .commit_manager
        .borrow_mut()
        .stage_new_holon(descriptor.clone())?;

    staged_reference
        .add_related_holons(
            context,
            RelationshipName(MapString("COMPONENT_OF".to_string())),
            vec![schema.clone()])?;

    if let Some(descriptor_ref) = header.described_by {
        staged_reference
            .add_related_holons(
                context,
                RelationshipName(MapString("DESCRIBED_BY".to_string())),
                vec![descriptor_ref])?
    };
    if let Some(is_subtype_of_ref) = header.is_subtype_of {
        staged_reference
            .add_related_holons(
                context,
                RelationshipName(MapString("IS_SUBTYPE_OF".to_string())),
                vec![is_subtype_of_ref])?
    };
    if let Some(owned_by_ref) = header.owned_by {
        staged_reference
            .add_related_holons(
                context,
                RelationshipName(MapString("OWNED_BY".to_string())),
                vec![owned_by_ref])?
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

pub fn derive_descriptor_name(type_name: &MapString) -> MapString {
    MapString(format!("{}{}", type_name.0, "Descriptor".to_string()))
}
