use holons_core::{dances::{DanceRequest, DanceType, RequestBody}, HolonReference};
use base_types::MapString;
use core_types::HolonError;
use integrity_core_types::RelationshipName;

///
/// Builds a DanceRequest for adding related holons to a source_holon.
pub fn build_add_related_holons_dance_request(
    holon_reference: HolonReference,
    relationship_name: RelationshipName,
    holons_to_add: Vec<HolonReference>,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_target_holons(relationship_name, holons_to_add);
    Ok(DanceRequest::new(
        MapString("add_related_holons".to_string()),
        DanceType::CommandMethod(holon_reference),
        body,
        None,
    ))
}
