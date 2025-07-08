use holons_core::{reference_layer::StagedReference, dances::{DanceRequest, DanceType, RequestBody}};
use base_types::MapString;
use core_types::HolonError;

///
/// Builds a DanceRequest for abandoning changes to a staged Holon.
pub fn build_abandon_staged_changes_dance_request(
    staged_reference: StagedReference,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(
        MapString("abandon_staged_changes".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        None,
    ))
}
