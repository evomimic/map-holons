use crate::descriptor_types::CoreSchemaPropertyTypeName::{MaxLength, MinLength};
use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName};
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};
use holons_core::core_shared_objects::holon::TransientHolon;
use holons_core::core_shared_objects::stage_new_holon_api;
use holons_core::core_shared_objects::HolonError;
use holons_core::{HolonReference, WriteableHolon, HolonsContextBehavior, StagedReference};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{BaseTypeKind, TypeKind};
use integrity_core_types::PropertyName;
use CoreSchemaPropertyTypeName::TypeName;

pub struct StringTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub type_name: MapString,
    pub min_length: MapInteger,
    pub max_length: MapInteger,
}

/// This function defines and stages (but does not persist) a new StringValueType
/// Values for each of its properties will be set based on supplied parameters.
///
/// *Naming Rule*:
///     `descriptor_name`:= `<type_name>"ValueDescriptor"`
///
/// The descriptor will have the following relationships populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonDescriptor (if supplied)
///
pub fn define_string_type(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    definition: StringTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let type_descriptor_ref = define_type_descriptor(
        context,
        schema,
        TypeKind::Value(BaseTypeKind::String),
        definition.header,
    )?;

    let mut string_type = TransientHolon::new();

    // Add its properties

    string_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(definition.type_name.clone())),
        )?
        .with_property_value(
            TypeName.as_property_name(),
            Some(BaseValue::StringValue(definition.type_name.clone())),
        )?
        .with_property_value(
            MinLength.as_property_name(),
            Some(BaseValue::IntegerValue(definition.min_length)),
        )?
        .with_property_value(
            MaxLength.as_property_name(),
            Some(BaseValue::IntegerValue(definition.max_length)),
        )?;

    // Stage new string type
    let string_type_ref = stage_new_holon_api(context, string_type.clone())?;

    // Add some relationships
    string_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(type_descriptor_ref)],
    )?;

    Ok(string_type_ref)
}
