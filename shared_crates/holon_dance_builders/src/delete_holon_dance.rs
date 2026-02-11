use base_types::MapString;
use core_types::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use integrity_core_types::LocalId;

/// Builds a DanceRequest for deleting a local Holon from the persistent store
pub fn build_delete_holon_dance_request(
    holon_to_delete: LocalId,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(
        MapString("delete_holon".to_string()),
        DanceType::DeleteMethod(holon_to_delete),
        body,
    ))
}
