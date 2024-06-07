// This file defines the TypeDescriptor struct and the dance functions it supports

use holons::context::HolonsContext;
use holons::staged_reference::{StagedReference};
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;

// use holons::relationship::{RelationshipName, RelationshipTarget};

use crate::semantic_version::set_semantic_version;
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
    context: &HolonsContext,
    schema: &HolonReference, // Type-COMPONENT_OF->Schema
    descriptor_name: MapString,
    type_name: MapString,
    base_type: BaseType,
    description: MapString,
    label: MapString, // Human-readable name for this type
    is_dependent: MapBoolean,
    is_value_descriptor: MapBoolean,
    described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    is_subtype_of: Option<HolonReference>, // Type-IS_SUBTYPE_OF->Type
    //_owned_by: HolonReference, // Holon-OWNED_BY->HolonSpace
) -> Result<StagedReference, HolonError> {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();
    // Define a default semantic_version as a String Property
    let version = MapString(set_semantic_version(0, 0, 1).to_string());

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(type_name),
        )
        .with_property_value(
            PropertyName(MapString("descriptor_name".to_string())),
            BaseValue::StringValue(descriptor_name),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(description),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(label),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::EnumValue(MapEnumValue(MapString(base_type.to_string()))),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(is_dependent),
        )
        .with_property_value(
            PropertyName(MapString("is_value_descriptor".to_string())),
            BaseValue::BooleanValue(is_value_descriptor),
        )
        .with_property_value(
            PropertyName(MapString("version".to_string())),
            BaseValue::StringValue(version),
        );

    // Stage the new TypeDescriptor
    let staged_reference = context
        .commit_manager
        .borrow_mut()
        .stage_new_holon(descriptor.clone())?;

    staged_reference
        .add_related_holons(
            context,
            RelationshipName(MapString("COMPONENT_OF".to_string())),
            vec![schema.clone()])?;

    if let Some(descriptor_ref) = described_by {
        staged_reference
            .add_related_holons(
                context,
                RelationshipName(MapString("DESCRIBED_BY".to_string())),
                vec![descriptor_ref])
    };
    if let Some(is_subtype_of_ref) = is_subtype_of {
        staged_reference
            .add_related_holons(
                context,
                RelationshipName(MapString("IS_SUBTYPE_OF".to_string())),
                vec![is_subtype_of_ref])
    };

    staged_reference
}

pub fn derive_descriptor_name(type_name: &MapString) -> MapString {
    MapString(format!("{}{}", type_name.0, "Descriptor".to_string()))
}
