use base_types::MapString;
use core_types::HolonError;
use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    reference_layer::HolonReference,
};
use integrity_core_types::PropertyMap;

///
/// Builds a DanceRequest for removing a new property value(s) to an transient or staged holon.
pub fn build_remove_properties_dance_request(
    holon_reference: HolonReference,
    properties: PropertyMap,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_parameter_values(properties);

    Ok(DanceRequest::new(
        MapString("remove_properties".to_string()),
        DanceType::CommandMethod(holon_reference),
        body,
        None,
    ))
}
