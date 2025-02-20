use holons_core::core_shared_objects::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody, SessionState};
use holons_core::StagedReference;
use shared_types_holon::{MapString, PropertyMap};
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
