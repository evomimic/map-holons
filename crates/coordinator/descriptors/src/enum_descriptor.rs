use hdi::prelude::debug;
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::space_manager::HolonStagingBehavior;
use holons::staged_reference::StagedReference;
use shared_types_holon::value_types::{BaseType, BaseValue, MapString, ValueType};
use shared_types_holon::PropertyName;

use crate::descriptor_types::CoreSchemaPropertyTypeName::TypeName;
use crate::descriptor_types::CoreSchemaRelationshipTypeName;
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};

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
    context: &HolonsContext,
    schema: &HolonReference,
    definition: EnumTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  STAGE A NEW ENUM TYPE DESCRIPTOR -------------------------------
    let enum_type_descriptor_ref = define_type_descriptor(
        context,
        schema,
        BaseType::Value(ValueType::Enum),
        definition.header,
    )?;

    // Build the new type

    let mut enum_type = Holon::new();

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

    let enum_type_ref = context.space_manager.borrow().stage_new_holon(enum_type.clone())?;

    // Add its relationships

    enum_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(enum_type_descriptor_ref)],
    )?;

    // Add the variants to the EnumType
    enum_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::Variants.as_rel_name(),
        definition.variants,
    )?;

    Ok(enum_type_ref)
}
