use dances_core::{dance_request::{DanceRequest, DanceType, RequestBody}, session_state::SessionState};
use holons_core::core_shared_objects::HolonError;
use shared_types_holon::{MapString, PropertyMap};
use holons_core::StagedReference;
///
/// Builds a DanceRequest for adding a new property value(s) to an already staged holon.
pub fn build_with_properties_dance_request(
    session_state: &SessionState,
    staged_reference: StagedReference,
    properties: PropertyMap,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_parameter_values(properties);

    Ok(DanceRequest::new(
        MapString("with_properties".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        session_state.clone(),
    ))
}