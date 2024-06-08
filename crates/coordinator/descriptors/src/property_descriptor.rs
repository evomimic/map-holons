use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::{BaseType, PropertyName};
use shared_types_holon::value_types::{MapBoolean, MapString};

use crate::type_descriptor::define_type_descriptor;

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
pub fn define_property_type(
    context: &HolonsContext,
    schema: &HolonReference,
    description: MapString,
    label: MapString, // Human-readable name for this type
    has_supertype: Option<HolonReference>,
    described_by: Option<HolonReference>,
    owned_by: Option<HolonReference>,
    property_name: PropertyName,
    property_of: HolonReference,
    value_type: HolonReference,
) -> Result<StagedReference, HolonError> {



    // build the type_name for the PropertyDescriptor
    let type_name = MapString(format!("{}_Property", property_name.0));

    let staged_reference = define_type_descriptor(
        context,
        schema,
        MapString(format!("{}{}", type_name.0, "PropertyDescriptor".to_string())),
        type_name,
        BaseType::Property,
        description,
        label,
        MapBoolean(false),
        MapBoolean(false),
        has_supertype,
        described_by,
        owned_by,
    )?;

    // Populate the relationships

    staged_reference
        .add_related_holons(
            context,
            RelationshipName(MapString("PROPERTY_OF".to_string())),
            vec![property_of.clone()])?;

    staged_reference
        .add_related_holons(
            context,
            RelationshipName(MapString("VALUE_TYPE".to_string())),
            vec![value_type.clone()])?;


    Ok(staged_reference)

}