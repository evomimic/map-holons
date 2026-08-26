use base_types::MapString;
use core_types::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody};

/// Builds a DanceRequest for retrieving the holons owned by the current HolonSpace
pub fn build_get_all_holons_dance_request() -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(MapString("get_all_holons".to_string()), DanceType::Standalone, body))
}
