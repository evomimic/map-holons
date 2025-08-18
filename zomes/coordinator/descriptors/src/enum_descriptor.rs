use hdi::prelude::debug;

use crate::descriptor_types::{
    CoreSchemaPropertyTypeName::TypeName, CoreSchemaRelationshipTypeName,
};
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};
use base_types::{BaseValue, MapString};
use core_types::{BaseTypeKind, HolonError, TypeKind};
use holons_core::{
    core_shared_objects::{stage_new_holon_api, TransientHolon},
    HolonReference, HolonsContextBehavior, StagedReference, WriteableHolon,
};
use integrity_core_types::PropertyName;

pub struct EnumTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub type_name: MapString,
    pub variants: Vec<HolonReference>,
}

/// This function defines and stages (but does not persist) a new EnumValueType
/// It sets for each of its properties based on supplied parameters.
/// It creates EnumVariantDescriptors for each of the variant strings supplied.
/// NOTE: the order of the strings in the variants vector is significant. It
/// is used to assign the variant_order within the corresponding variant descriptor.
///
/// *Naming Rule*:
///     `descriptor_name`:= `<type_name>"ValueDescriptor"`
///
/// The descriptor will have the following relationships populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonDescriptor (if supplied)
/// * VARIANTS -> EnumVariantDescriptor (based on variants supplied)
/// *
///
pub fn define_enum_type(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    definition: EnumTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  STAGE A NEW ENUM TYPE DESCRIPTOR -------------------------------
    let enum_type_descriptor_ref = define_type_descriptor(
        context,
        schema,
        TypeKind::Value(BaseTypeKind::Enum),
        definition.header,
    )?;

    // Build the new type

    let mut enum_type = TransientHolon::new();

    // Add its properties

    enum_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(definition.type_name.clone()),
        )?
        .with_property_value(
            TypeName.as_property_name(),
            BaseValue::StringValue(definition.type_name),
        )?;

    // Stage the type

    debug!("Staging... {:#?}", enum_type.clone());

    let enum_type_ref = stage_new_holon_api(context, enum_type.clone())?;

    // Add its relationships

    enum_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor,
        vec![HolonReference::Staged(enum_type_descriptor_ref)],
    )?;

    // Add the variants to the EnumType
    enum_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::Variants,
        definition.variants,
    )?;

    Ok(enum_type_ref)
}
