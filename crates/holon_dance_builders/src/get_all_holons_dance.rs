use holons_core::core_shared_objects::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use base_types::MapString;

/// Builds a DanceRequest for retrieving all holons from the persistent store
pub fn build_get_all_holons_dance_request() -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(
        MapString("get_all_holons".to_string()),
        DanceType::Standalone,
        body,
        None,
    ))
}
