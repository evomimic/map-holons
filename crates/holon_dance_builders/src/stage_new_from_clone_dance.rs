use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use holons_core::{core_shared_objects::HolonError, HolonReference};
use shared_types_holon::MapString;

///
/// Builds a dance request for staging a new cloned Holon
pub fn build_stage_new_from_clone_dance_request(
    holon_reference: HolonReference,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        MapString("stage_new_from_clone".to_string()),
        DanceType::CloneMethod(holon_reference),
        RequestBody::None,
        None,
    ))
}
