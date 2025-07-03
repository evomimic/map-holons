use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use base_types::MapString;
use core_types::HolonError;

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_commit_dance_request() -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(MapString("commit".to_string()), DanceType::Standalone, body, None))
}
