use base_types::MapString;
use core_types::HolonError;
use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    HolonReference,
};

///
/// Builds a DanceRequest for abandoning changes to a staged Holon.
pub fn build_abandon_staged_changes_dance_request(
    staged_reference: HolonReference,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(
        MapString("abandon_staged_changes".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
    ))
}
