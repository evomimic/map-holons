// This file defines the TypeDescriptor struct and the dance functions it supports

use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::staged_reference::StagedReference;

// use holons::relationship::{RelationshipName, HolonCollection};

use crate::descriptor_types::TypeDescriptor;
use crate::semantic_version::define_semantic_version;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::value_types::{BaseType, BaseValue, MapBoolean, MapEnumValue, MapString};

/// This is a helper function that defines and stages (but does not commit) a new TypeDescriptor.
/// It is intended to be called by other define_xxx_descriptor functions
///
/// Values for each of the TypeDescriptor _properties_ will be set based on supplied parameters.
///
/// The descriptor will have the following _relationships_ populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE->TypeDescriptor (if supplied)
///
///
pub fn define_type_descriptor(
    _context: &HolonsContext,
    _schema: StagedReference,
    descriptor_name: MapString,
    type_name: MapString,
    base_type: BaseType,
    description: MapString,
    label: MapString, // Human readable name for this type
    is_dependent: MapBoolean,
    is_value_descriptor: MapBoolean,
    _described_by: Option<StagedReference>,
    _has_supertype: Option<StagedReference>,
    //_owned_by: HolonReference, // HolonSpace
) -> Result<TypeDescriptor, HolonError> {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();
    // let schema_reference = StagedReference::from_holon()from_holon(schema.0.clone()));
    // let schema_target = HolonCollection::One(schema_reference);

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
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
            BaseValue::BooleanValue(is_value_descriptor),
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
