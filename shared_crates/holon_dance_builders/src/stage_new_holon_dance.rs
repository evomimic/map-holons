use base_types::MapString;
use core_types::HolonError;
use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    reference_layer::TransientReference,
};

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_stage_new_holon_dance_request(
    holon: TransientReference,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::TransientReference(holon);
    Ok(DanceRequest::new(
        MapString("stage_new_holon".to_string()),
        DanceType::Standalone,
        body,
    ))
}
