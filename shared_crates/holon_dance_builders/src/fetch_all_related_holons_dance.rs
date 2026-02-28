use base_types::MapString;
use core_types::HolonError;
use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    query_layer::NodeCollection,
};

/// Builds a DanceRequest for fetching all related holons for each source node.
pub fn build_fetch_all_related_holons_dance_request(
    node_collection: NodeCollection,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        MapString("fetch_all_related_holons".to_string()),
        DanceType::QueryMethod(node_collection),
        RequestBody::None,
    ))
}
