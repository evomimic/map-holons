use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use base_types::MapString;
use core_types::{HolonError, HolonId};

///
/// Builds a dance request for staging a new cloned Holon
pub fn build_stage_new_version_dance_request(
    holon_id: HolonId,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        MapString("stage_new_version".to_string()),
        DanceType::NewVersionMethod(holon_id),
        RequestBody::None,
        None,
    ))
}
