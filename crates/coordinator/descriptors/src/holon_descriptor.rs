
use crate::type_descriptor::{define_type_descriptor};


use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;
use shared_types_holon::value_types::{MapBoolean, MapString};
use crate::descriptor_types::{HolonDescriptor, Schema, TypeDescriptor};

/// This function defines and stages (but does not persist) a new HolonDescriptor.
/// Values for each of the HolonDescriptor properties will be set based on supplied parameters.
///
/// *Naming Rule*:
///     `descriptor_name`:= `<type_name>"HolonDescriptor"`
///
/// The descriptor will have the following relationships populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonDescriptor (if supplied)
///
pub fn define_holon_descriptor(
    schema: &Schema,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    has_supertype: Option<&TypeDescriptor>,
    described_by: Option<&TypeDescriptor>,

) -> HolonDescriptor {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------

    let descriptor = define_type_descriptor(
        schema,
        MapString(format!("{}{}", type_name.0, "HolonDescriptor".to_string())),
        type_name,
        BaseTypeHolon,
        description,
        label,
        MapBoolean(false),
        MapBoolean(false),
        has_supertype,
        described_by,
    );



    HolonDescriptor(descriptor.0)
}
