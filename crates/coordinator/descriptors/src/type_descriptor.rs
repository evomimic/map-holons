// This file defines the TypeDescriptor struct and the dance functions it supports

use hdk::prelude::{info,debug,trace,warn};
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use crate::descriptor_types::TypeDescriptor;
use crate::semantic_version::define_semantic_version;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapEnumValue, MapString};

use crate::semantic_version::SemanticVersion;

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
    descriptor_name: MapString,
    type_name: MapString,
    base_type: BaseType,
    description: MapString,
    label: MapString, // Human-readable name for this type
    is_dependent: MapBoolean,
    is_value_type: MapBoolean,
    described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    is_subtype_of: Option<HolonReference>, // Type-IS_SUBTYPE_OF->Type
    owned_by: Option<HolonReference>, // Holon-OWNED_BY->HolonSpace
) -> Result<StagedReference, HolonError> {

    info!("Staging... {:#?}", type_name.0.clone());

    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();
  
    // Define a default semantic_version as a String Property
    let initial_version = MapString(SemanticVersion::default().to_string());


    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(type_name.clone()),
        )?
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(type_name),
        )?
        .with_property_value(
            PropertyName(MapString("descriptor_name".to_string())),
            BaseValue::StringValue(descriptor_name),
        )?
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(description),
        )?
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(label),
        )?
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::EnumValue(MapEnumValue(MapString(base_type.to_string()))),
        )?
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(is_dependent),
        )?
        .with_property_value(
            PropertyName(MapString("is_value_descriptor".to_string())),
            BaseValue::BooleanValue(is_value_type),

        )?
        .with_property_value(
            PropertyName(MapString("version".to_string())),
            BaseValue::StringValue(initial_version),
        )?;


    // Define a default semantic_version
    let _version = define_semantic_version(0, 0, 1);

    // Add the outbound relationships shared by all TypeDescriptors
    // let version_target = define_local_target(&version);

    // descriptor
    //     .add_related_holon(
    //         RelationshipName(MapString("COMPONENT_OF".to_string())),
    //         schema_target,
    //     )
    //     .add_related_holon(
    //         RelationshipName(MapString("VERSION".to_string())),
    //         version_target,
    //     );

    // TODO: If has_supertype is supplied, populate that relationship

    // if let Some(supertype) = has_supertype  {
    //     let supertype_reference = HolonReference::Local(LocalHolonReference::from_holon(supertype.0.clone()));
    //     descriptor.add_related_holon(
    //         RelationshipName(MapString("HAS_SUPERTYPE".to_string())),
    //         HolonCollection::ZeroOrOne(Some(supertype_reference)),
    //     );
    // }
    // // TODO: If described_by is supplied, populate that relationship
    // if let Some(is_described_by) = described_by  {
    //     let described_by_reference = HolonReference::Local(LocalHolonReference::from_holon(is_described_by.0.clone()));
    //
    //     descriptor
    //         .add_related_holon(
    //         RelationshipName(MapString("DESCRIBED_BY".to_string())),
    //         HolonCollection::ZeroOrOne(Some(described_by_reference)),
    //     );
    // }
    //TODO: Populate owned_by relationship
    // descriptor.add_related_holon(
    //     RelationshipName(MapString("OWNED_BY".to_string())),
    //     owned_by.clone(),

    Ok(TypeDescriptor(descriptor))

}

pub fn derive_descriptor_name(type_name: &MapString) -> MapString {
    MapString(format!("{}{}", type_name.0, "Descriptor".to_string()))
}
