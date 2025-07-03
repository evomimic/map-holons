use holons_core::{core_shared_objects::RelationshipName, dances::{DanceRequest, DanceType, RequestBody}, HolonReference, StagedReference};
use base_types::MapString;
use core_types::HolonError;

///
/// Builds a DanceRequest for adding related holons to a source_holon.
pub fn build_add_related_holons_dance_request(
    staged_reference: StagedReference,
    relationship_name: RelationshipName,
    holons_to_add: Vec<HolonReference>,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_target_holons(relationship_name, holons_to_add);
    Ok(DanceRequest::new(
        MapString("add_related_holons".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        None,
    ))
}
