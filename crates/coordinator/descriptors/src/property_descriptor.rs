use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::BaseType;
use shared_types_holon::value_types::MapString;

use crate::type_descriptor::{define_type_descriptor, TypeDefinitionHeader};

/// This function defines and stages (but does not persist) a new PropertyDescriptor.
/// Values for each of the PropertyDescriptor properties will be set based on supplied parameters.
///
/// *Naming Rules:*
/// * `type_name` will be automatically derived based on the following rule:
///     `<property_name>"_PROPERTY_OF_"<type_name of the HolonDescriptor it is a PROPERTY_OF>`
/// *  `descriptor_name` will be derived by appending `Descriptor` to its type_name
///
/// The descriptor will have the following relationships populated:
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * PROPERTY_OF->HolonDescriptor (supplied)
/// * VALUE_TYPE->ValueDescriptor (supplied)
///
///
pub struct PropertyTypeDefinition {
    header: TypeDefinitionHeader,
    property_of: HolonReference, // HolonType
    value_type: HolonReference, // ValueType
}
pub fn define_property_type(
    context: &HolonsContext,
    schema: &HolonReference,
    definition: PropertyTypeDefinition,
) -> Result<StagedReference, HolonError> {

    let staged_reference = define_type_descriptor(
        context,
        schema,
        BaseType::Property,
        definition.header,
    )?;

    // Populate the relationships

    staged_reference
        .add_related_holons(
            context,
            RelationshipName(MapString("PROPERTY_OF".to_string())),
            vec![definition.property_of.clone()])?;

    staged_reference
        .add_related_holons(
            context,
            RelationshipName(MapString("VALUE_TYPE".to_string())),
            vec![definition.value_type.clone()])?;


    Ok(staged_reference)

}