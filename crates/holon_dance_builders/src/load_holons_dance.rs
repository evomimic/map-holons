// crates/holon_dance_builders/src/load_holons_dance.rs

use base_types::MapString;
use core_types::HolonError;
use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    reference_layer::TransientReference,
};

/// Build a Standalone dance request to load holons from a HolonLoadSet.
/// NOTE: We can't assume descriptors are loaded here; validation happens in the guest.
pub fn build_load_holons_dance_request(
    bundle_set: TransientReference,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        MapString("load_holons".into()),
        DanceType::Standalone,
        RequestBody::TransientReference(bundle_set),
        None,
    ))
}
