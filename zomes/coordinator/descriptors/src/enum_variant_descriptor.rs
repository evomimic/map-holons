//use std::env::var;

use hdi::prelude::debug;

use crate::descriptor_types::CoreSchemaPropertyTypeName::{TypeName, VariantOrder};
use crate::descriptor_types::CoreSchemaRelationshipTypeName;
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};
use holons_core::{core_shared_objects::{TransientHolon, stage_new_holon_api}, HolonReference, WriteableHolon, HolonsContextBehavior, StagedReference};
use base_types::{BaseValue, MapInteger, MapString};
use core_types::{TypeKind, BaseTypeKind, HolonError};
use integrity_core_types::PropertyName;

pub struct EnumVariantTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub type_name: MapString, // unique variant name
    pub variant_order: MapInteger,
}

/// This function defines and stages (but does not persist) a new EnumVariantType
/// It sets each of its properties based on supplied parameters.
/// EnumVariants do not own any relationships other than TYPE_DESCRIPTOR (the relationship from an EnumType to its variants
/// is owned by the EnumType).
///
/// *Naming Rule*:
///     `descriptor_name`:= `<type_name>"ValueDescriptor"`
///
/// The descriptor will have the following relationships populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonDescriptor (if supplied)

pub fn define_enum_variant_type(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    definition: EnumVariantTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  STAGE A NEW ENUM VARIANT TYPE DESCRIPTOR -------------------------------
    let enum_variant_type_descriptor_ref = define_type_descriptor(
        context,
        schema,
        TypeKind::Value(BaseTypeKind::Enum),
        definition.header,
    )?;

    // Build the new type

    let mut enum_variant_type = TransientHolon::new();

    // Add its properties

    enum_variant_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(definition.type_name.clone()),
        )?
        .with_property_value(
            TypeName.as_property_name(),
            BaseValue::StringValue(definition.type_name.clone()),
        )?
        .with_property_value(
            VariantOrder.as_property_name(),
            BaseValue::IntegerValue(definition.variant_order),
        )?;

    // Stage the type

    debug!("Staging... {:#?}", enum_variant_type.clone());

    let enum_variant_type_ref = stage_new_holon_api(context, enum_variant_type.clone())?;

    // Add some relationships

    enum_variant_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor,
        vec![HolonReference::Staged(enum_variant_type_descriptor_ref)],
    )?;

    Ok(enum_variant_type_ref)
}
