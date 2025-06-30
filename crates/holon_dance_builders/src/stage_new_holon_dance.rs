use base_types::MapString;
use holons_core::core_shared_objects::{HolonError, TransientHolon};
use holons_core::dances::{DanceRequest, DanceType, RequestBody};

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
/// TODO: should the input parameter be Holon?
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
