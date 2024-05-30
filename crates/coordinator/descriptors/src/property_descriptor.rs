use crate::descriptor_types::PropertyDescriptor;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;
use shared_types_holon::value_types::{MapBoolean, MapString};

use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};

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
pub fn define_property_descriptor(
    context: &HolonsContext,
    schema: StagedReference,
    property_name: MapString, // snake_case name for this property, e.g., "name" -- TODO: define PropertyName StringValueType
    description: MapString,
    label: MapString,             // Human readable name for this property name
    _property_of: HolonReference, // TODO: Change this type to HolonReference once fn's to get_holon from reference are available
    _value_type: HolonReference, // TODO: Change this type to HolonReference once fn's to get_holon from reference are available
    has_supertype: Option<StagedReference>,
    described_by: Option<StagedReference>,
) -> Result<PropertyDescriptor, HolonError> {
    let property_of_name =
        MapString("TODO: Extract type_name from the PROPERTY_OF HolonDescriptor".to_string());

    // build the type_name for the PropertyDescriptor
    let type_name = MapString(format!(
        "{}_PROPERTY_OF_{}",
        property_name.0, property_of_name.0
    ));

    let descriptor = define_type_descriptor(
        context,
        schema,
        derive_descriptor_name(&property_name),
        type_name,
        BaseTypeHolon, // Do we need a Property BaseType???
        description,
        label,
        MapBoolean(true),
        MapBoolean(false),
        described_by,
        has_supertype,
    )?;

    // Populate the relationships

    // descriptor
    //     .add_related_holon(
    //         RelationshipName(MapString("COMPONENT_OF".to_string())),
    //         schema.clone(),
    //     )
    //     .add_related_holon(
    //         RelationshipName(MapString("PROPERTY_OF".to_string())),
    //         property_of,
    //     )
    //     .add_related_holon(
    //         RelationshipName(MapString("VALUE_TYPE".to_string())),
    //         value_type,
    //     );

    Ok(PropertyDescriptor(descriptor.0))
}
