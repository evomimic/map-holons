use holons_core::core_shared_objects::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody, SessionState};
use shared_types_holon::{LocalId, MapString};

/// Builds a DanceRequest for deleting a local Holon from the persistent store
pub fn build_delete_holon_dance_request(
    session_state: &SessionState,
    holon_to_delete: LocalId,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(
        MapString("delete_holon".to_string()),
        DanceType::DeleteMethod(holon_to_delete),
        body,
        session_state.clone(),
    ))
}
