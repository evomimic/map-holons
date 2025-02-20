use holons_core::core_shared_objects::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody, SessionState};
use shared_types_holon::{HolonId, MapString};

/// Builds a DanceRequest for retrieving holon by HolonId from the persistent store
pub fn build_get_holon_by_id_dance_request(
    session_state: &SessionState,
    holon_id: HolonId,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::HolonId(holon_id);
    Ok(DanceRequest::new(
        MapString("get_holon_by_id".to_string()),
        DanceType::Standalone,
        body,
        session_state.clone(),
    ))
}
