use dances_core::{dance_request::{DanceRequest, DanceType, RequestBody}, session_state::SessionState};
use holons_core::{core_shared_objects::{HolonError, RelationshipName}, HolonReference, StagedReference};
use shared_types_holon::MapString;

/// Builds a DanceRequest for removing related holons to a source_holon.
pub fn build_remove_related_holons_dance_request(
    session_state: &SessionState,
    staged_reference: StagedReference,
    relationship_name: RelationshipName,
    holons_to_remove: Vec<HolonReference>,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_target_holons(relationship_name, holons_to_remove);
    Ok(DanceRequest::new(
        MapString("remove_related_holons".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        session_state.clone(),
    ))
}