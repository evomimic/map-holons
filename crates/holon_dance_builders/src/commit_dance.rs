use dances_core::{dance_request::{DanceRequest, DanceType, RequestBody}, session_state::SessionState};
use holons_core::core_shared_objects::HolonError;
use shared_types_holon::MapString;

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_commit_dance_request(
    session_state: &SessionState,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(
        MapString("commit".to_string()),
        DanceType::Standalone,
        body,
        session_state.clone(),
    ))
}