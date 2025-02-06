use dances_core::{dance_request::{DanceRequest, DanceType, RequestBody}, session_state::SessionState};
use holons_core::{core_shared_objects::HolonError, HolonReference};
use shared_types_holon::MapString;

///
/// Builds a dance request for staging a new cloned Holon
pub fn build_stage_new_from_clone_dance_request(
    session_state: &SessionState,
    holon_reference: HolonReference,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        MapString("stage_new_from_clone".to_string()),
        DanceType::CloneMethod(holon_reference),
        RequestBody::None,
        session_state.clone(),
    ))
}