use holons_core::core_shared_objects::HolonError;
use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use holons_core::query_layer::{NodeCollection, QueryExpression};
use base_types::MapString;

///
/// Builds a DanceRequest for getting related holons optionally filtered by relationship name.
pub fn build_query_relationships_dance_request(
    node_collection: NodeCollection,
    query_expression: QueryExpression,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_query_expression(query_expression);
    Ok(DanceRequest::new(
        MapString("query_relationships".to_string()),
        DanceType::QueryMethod(node_collection),
        body,
        None,
    ))
}
