use base_types::MapString;
use core_types::HolonError;
use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    query_layer::{NodeCollection, QueryExpression},
};

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
