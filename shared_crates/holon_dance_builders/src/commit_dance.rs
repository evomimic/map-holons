use base_types::MapString;
use core_types::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody};

///
/// Builds a DanceRequest for attempting a commit of StagedHolons.
pub fn build_commit_dance_request() -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(MapString("commit".to_string()), DanceType::Standalone, body))
}
