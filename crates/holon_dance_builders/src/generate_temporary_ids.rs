use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    HolonError,
};
use shared_types_holon::{MapInteger, MapString};

///
/// Builds a DanceRequest for generating a batch of temporary ids from guest side.
pub fn build_generate_temporary_ids_dance_request(
    amount: MapInteger,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(
        MapString("generate_temporary_ids".to_string()),
        DanceType::GenerateTemporaryIds(amount),
        body,
        None,
    ))
}
