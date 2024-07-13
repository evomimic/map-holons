use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::BaseType;
use shared_types_holon::value_types::MapString;

use crate::type_descriptor::{define_type_descriptor, TypeDefinitionHeader};

pub struct HolonDefinition {
    pub header:TypeDefinitionHeader,
    pub properties: Vec<HolonReference>,
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
    context: &HolonsContext,
    schema: &HolonReference,
    definition: HolonDefinition,
) -> Result<StagedReference, HolonError> {


    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------

    let descriptor = define_type_descriptor(
        context,
        schema,
        BaseType::Holon,
        definition.header,
    )?;
    if definition.properties.len() > 0 {
        descriptor
            .add_related_holons(
                context,
                RelationshipName(MapString("PROPERTIES".to_string())),
                definition.properties)?;
    }

    Ok(descriptor)
}
