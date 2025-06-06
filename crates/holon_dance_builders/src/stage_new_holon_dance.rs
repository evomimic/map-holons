use holons_core::core_shared_objects::{Holon, HolonError};
use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use base_types::MapString;

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_stage_new_holon_dance_request(holon: Holon) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_holon(holon);
    Ok(DanceRequest::new(
        MapString("stage_new_holon".to_string()),
        DanceType::Standalone,
        body,
        None,
    ))
}
