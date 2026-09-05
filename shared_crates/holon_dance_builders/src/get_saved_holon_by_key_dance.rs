use base_types::MapString;
use core_types::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody};

/// Builds a dance request that resolves one exact key to the sole visible saved lineage head.
pub fn build_get_saved_holon_by_key_dance_request(
    key: MapString,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        MapString("get_saved_holon_by_key".to_string()),
        DanceType::Standalone,
        RequestBody::Key(key),
    ))
}
