use holons::context::HolonsContext;

use holons::holon_error::HolonError;

use holons::holon_reference::HolonReference;


use crate::type_descriptor::define_type_descriptor;
use holons::staged_reference::StagedReference;

use crate::descriptor_types::HolonDescriptor;
use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;
use shared_types_holon::value_types::{MapBoolean, MapString};

use crate::descriptor_types::{HolonType};


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
pub fn define_holon_type(
    context: &HolonsContext,
    schema: HolonReference,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    has_supertype: Option<StagedReference>,
    described_by: Option<StagedReference>,
) -> Result<HolonType, HolonError> {

    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------

    let descriptor = define_type_descriptor(
        context,
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
    )?;


    Ok(HolonType(descriptor.0))

}
