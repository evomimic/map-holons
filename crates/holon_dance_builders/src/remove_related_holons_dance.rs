use base_types::MapString;
use core_types::HolonError;
use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    HolonReference,
};
use integrity_core_types::RelationshipName;

/// Builds a DanceRequest for removing related holons to a source_holon.
pub fn build_remove_related_holons_dance_request(
    holon_reference: HolonReference,
    relationship_name: RelationshipName,
    holons_to_remove: Vec<HolonReference>,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_target_holons(relationship_name, holons_to_remove);
    Ok(DanceRequest::new(
        MapString("remove_related_holons".to_string()),
        DanceType::CommandMethod(holon_reference),
        body,
        None,
    ))
}
