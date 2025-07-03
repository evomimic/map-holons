use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use base_types::MapString;
use core_types::{HolonError, HolonId};

/// Builds a DanceRequest for retrieving holon by HolonId from the persistent store
pub fn build_get_holon_by_id_dance_request(holon_id: HolonId) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::HolonId(holon_id);
    Ok(DanceRequest::new(
        MapString("get_holon_by_id".to_string()),
        DanceType::Standalone,
        body,
        None,
    ))
}
