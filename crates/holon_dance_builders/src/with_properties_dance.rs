use holons_core::{dances::{DanceRequest, DanceType, RequestBody}, StagedReference};
use base_types::MapString;
use core_types::HolonError;
use integrity_core_types::PropertyMap;

///
/// Builds a DanceRequest for adding a new property value(s) to an already staged holon.
pub fn build_with_properties_dance_request(
    staged_reference: StagedReference,
    properties: PropertyMap,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_parameter_values(properties);

    Ok(DanceRequest::new(
        MapString("with_properties".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        None,
    ))
}
