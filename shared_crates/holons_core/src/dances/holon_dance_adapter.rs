//! Dance adapters for the holons zome.
//!
//! This module defines the `*_dance` adapter functions that execute individual
//! dances on the guest side. Each adapter:
//! 1. Extracts required input parameters from the `DanceRequest`
//! 2. Invokes the appropriate native/reference-layer function
//! 3. Produces a `ResponseBody` (or maps errors to `HolonError` for the dancer to wrap)
//!
//! Notes:
//! - **Request builders have moved** to the `holon_dance_builders` crate. This module no longer
//!   exposes `build_*` helper functions.
//! - These adapters intentionally insulate native/reference-layer code from the Dance protocol
//!   details and vice versa.
//! - Error mapping to `DanceResponse` status codes is handled by the dancer/dispatch layer;
//!   adapters return `Result<ResponseBody, HolonError>`.

use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::{debug, info};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::query_layer::{Node, NodeCollection, QueryPathMap};
use crate::reference_layer::TransientReference;
use crate::{
    dances::{
        dance_request::{DanceType, RequestBody},
        dance_response::ResponseBody,
        DanceRequest,
    },
    query_layer::evaluate_query,
    reference_layer::HolonReference,
    ReadableHolon,
};
use core_types::HolonError;

/// Commit all staged holons to the persistent store
///
/// *DanceRequest:*
/// - dance_name: "commit"
/// - dance_type: Standalone
/// - request_body: None
///
/// *ResponseBody:*
/// - Holons -- a vector of clones of all successfully committed holons
///
pub fn commit_dance(
    context: &Arc<TransactionContext>,
    _request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered commit_dance");
    let commit_response = context.commit()?;
    Ok(ResponseBody::HolonReference(commit_response.into()))
}

/// This dance deletes an existing holon from the persistent store.
///
/// *DanceRequest:*
/// - dance_name: "delete_holon"
/// - dance_type: DeleteMethod(HolonId)
/// - request_body: None
///
///
/// *ResponseBody:*
/// None
///
// In the absence of descriptors that can specify the DeletionSemantic,
// this enhancement will adopt Allow as a default policy. When we have RelationshipDescriptors, we can use their properties
// to drive a richer range of deletion behaviors.
//
// NOTE: This dance implements an immediate delete. We may want to consider staging holons for deletion and postponing the actual deletion until commit.
// This would allow the cascaded effects of deletion to be determined and shared with the agent, leaving them free to cancel the deletion if desired.
// A staged deletion process would be more consistent with the staged creation process.
pub fn delete_holon_dance(
    context: &Arc<TransactionContext>,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered delete_holon dance");
    match request.dance_type {
        DanceType::DeleteMethod(holon_id) => {
            // Call the new `delete_holon_api` function
            context.mutation().delete_holon(holon_id).map(|_| ResponseBody::None)
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected DeleteMethod(HolonId), didn't get one".to_string(),
        )),
    }
}

/// Get all holons from the persistent store
///
/// *DanceRequest:*
/// - dance_name: "get_all_holons"
/// - dance_type: Standalone
/// - request_body: None
///
/// *ResponseBody:*
/// - HolonCollection
///
pub fn get_all_holons_dance(
    context: &Arc<TransactionContext>,
    _request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // TODO: add support for descriptor parameter
    //
    //
    info!("----- Entered get_all_holons dance ----");
    Ok(ResponseBody::HolonCollection(context.lookup().get_all_holons()?))
}

/// Gets Holon from persistent store, located by HolonId
///
/// *DanceRequest:*
/// - dance_name: "get_holon_by_id"
/// - dance_type: Standalone
/// - request_body:
///     - HolonId(HolonId)
///
/// *ResponseBody:*
///     - Holon(Holon)
///
pub fn get_holon_by_id_dance(
    context: &Arc<TransactionContext>,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered get_holon_by_id_dance.");
    let holon_id = match request.body {
        RequestBody::HolonId(id) => id,
        _ => {
            return Err(HolonError::InvalidParameter(
                "RequestBody variant must be HolonId".to_string(),
            ))
        }
    };
    debug!("asking transaction context to fetch holon");
    let holon = context.fetch_holon_internal(&holon_id)?;

    let holon = holon.clone();
    Ok(ResponseBody::Holon(holon))
}

/// Executes the `"load_holons"` dance (guest side).
///
/// This adapter validates the request and delegates Holon loading to the
/// guest `HolonServiceApi` implementation, which calls the loader controller.
///
/// *DanceRequest:*
/// - dance_name: **"load_holons"**
/// - dance_type: **Standalone**
/// - request_body: **TransientReference(…HolonLoadSet…)**
///
/// *ResponseBody:*
/// - **HolonReference(HolonReference::Transient)** — a transient reference to the
///   `HolonLoadResponse` holon produced by the loader
///
/// # Errors
/// Returns `HolonError::InvalidParameter` if the `DanceType` is not `Standalone`
/// or if the `RequestBody` is not `TransientReference`.
///
pub fn load_holons_dance(
    context: &Arc<TransactionContext>,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered load holons dance");
    // Validate dance type
    if request.dance_type != DanceType::Standalone {
        return Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected Standalone".to_string(),
        ));
    }

    // Extract load set reference
    let load_set_reference: TransientReference = match request.body {
        RequestBody::TransientReference(transient_reference) => transient_reference,
        _ => {
            return Err(HolonError::InvalidParameter(
                "Invalid RequestBody: expected TransientReference (HolonLoadSet)".to_string(),
            ))
        }
    };

    // Terminal load path: context owns lifecycle transition on successful completion.
    let response_reference = context.load_holons_and_commit(load_set_reference)?;

    // Wrap transient response holon
    Ok(ResponseBody::HolonReference(HolonReference::Transient(response_reference)))
}

/// Query relationships
///
/// *DanceRequest:*
/// - dance_name: "query_relationships"
/// - dance_type: QueryMethod(NodeCollection) -- specifies the Collection to use as the source of the query
/// - request_body: QueryExpression(QueryExpression)
///     ///
/// *ResponseBody:*
/// - NodeCollection -- a collection containing the same source nodes passed in the request, but with their `relationships`
/// updated to include entries that satisfy the criteria set by the QueryExpression supplied in the request
///
pub fn query_relationships_dance(
    _context: &Arc<TransactionContext>,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("Entered query_relationships_dance");

    match request.dance_type {
        DanceType::QueryMethod(node_collection) => {
            let relationship_name =
                match request.body {
                    RequestBody::QueryExpression(expression) => expression.relationship_name,
                    _ => return Err(HolonError::InvalidParameter(
                        "Invalid RequestBody: expected QueryExpression with relationship name, \
                        didn't get one"
                            .to_string(),
                    )),
                };

            let result_collection = evaluate_query(node_collection, relationship_name)?;
            Ok(ResponseBody::NodeCollection(result_collection))
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected QueryMethod, didn't get one".to_string(),
        )),
    }
}

/// Fetch all relationships for each source node in the supplied query collection.
///
/// *DanceRequest:*
/// - dance_name: "fetch_all_related_holons"
/// - dance_type: QueryMethod(NodeCollection)
/// - request_body: None
///
/// *ResponseBody:*
/// - NodeCollection -- same source nodes with `relationships` populated for all relationship names.
pub fn fetch_all_related_holons_dance(
    _context: &Arc<TransactionContext>,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("Entered fetch_all_related_holons_dance");

    match request.dance_type {
        DanceType::QueryMethod(node_collection) => {
            if request.body != RequestBody::None {
                return Err(HolonError::InvalidParameter(
                    "Invalid RequestBody: expected None".to_string(),
                ));
            }

            let mut result_collection = NodeCollection::new_empty();
            result_collection.query_spec = node_collection.query_spec.clone();

            for node in node_collection.members {
                let relationship_map = node.source_holon.all_related_holons()?;
                let mut path_map = QueryPathMap::new(BTreeMap::new());

                for (relationship_name, collection_arc) in relationship_map.iter() {
                    let collection = collection_arc.read().map_err(|e| {
                        HolonError::FailedToAcquireLock(format!(
                            "Failed to acquire read lock on holon collection: {}",
                            e
                        ))
                    })?;

                    let mut related_nodes = NodeCollection::new_empty();
                    for reference in collection.get_members() {
                        related_nodes.members.push(Node::new(reference.clone(), None));
                    }

                    path_map.0.insert(relationship_name, related_nodes);
                }

                result_collection
                    .members
                    .push(Node::new(node.source_holon.clone(), Some(path_map)));
            }

            Ok(ResponseBody::NodeCollection(result_collection))
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected QueryMethod, didn't get one".to_string(),
        )),
    }
}
