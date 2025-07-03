use holons_core::{core_shared_objects::TransientHolon, dances::{DanceRequest, DanceType, RequestBody}};
use base_types::MapString;
use core_types::HolonError;

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_stage_new_holon_dance_request(
    holon: TransientHolon,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::TransientHolon(holon);
    Ok(DanceRequest::new(
        MapString("stage_new_holon".to_string()),
        DanceType::Standalone,
        body,
        None,
    ))
}
