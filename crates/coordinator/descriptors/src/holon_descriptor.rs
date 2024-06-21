use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::value_types::{MapBoolean, MapString};
use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;

use crate::type_descriptor::define_type_descriptor;

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
///
pub fn define_holon_type(
    context: &HolonsContext,
    schema: &HolonReference,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human-readable name for this type
    has_supertype: Option<HolonReference>,
    described_by: Option<HolonReference>,
    owned_by: Option<HolonReference>

) -> Result<StagedReference, HolonError> {


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
        owned_by,
    )?;



    Ok(descriptor)
}
