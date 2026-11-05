use base_types::MapString;
use core_types::HolonId;
use holons_core::dances::{DanceRequest, DanceType, RequestBody};

/// Builds an infrastructure-only exact saved-node hydration request.
pub fn build_get_holons_by_ids_dance_request(ids: Vec<HolonId>) -> DanceRequest {
    DanceRequest::new(
        MapString("get_holons_by_ids".into()),
        DanceType::Standalone,
        RequestBody::SavedHolonIds(ids),
    )
}
