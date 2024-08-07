//! This file defines the DancesAdaptors offered by the holons zome.
//! TODO: Move these adaptors to their own zome
//!
//! For each Dance, this file defines:
//! - a `build_` function as a helper function for creating DanceRequests for that Dance from
//! native parameters.
//!- a function that performs the dance
//!
//!
//! As a dance adaptor, this function wraps (and insulates) Dancer from native functionality
//! and insulates the native function from any dependency on Dances. In general, this means:
//! 1.  Extracting any required input parameters from the DanceRequest's request_body
//! 2.  Invoking the native function
//! 3.  Creating a DanceResponse based on the results returned by the native function. This includes,
//! mapping any errors into an appropriate ResponseStatus and returning results in the body.

// use std::borrow::Borrow;
// use std::rc::Rc;

use std::collections::BTreeMap;
use std::rc::Rc;

use derive_new::new;
use hdk::prelude::*;
use holons::commit_manager::CommitRequestStatus::*;
use holons::commit_manager::{CommitManager, StagedIndex};
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::{HolonGettable, HolonReference};
use holons::relationship::RelationshipName;
use shared_types_holon::HolonId;
use shared_types_holon::{MapString, PropertyMap};

use crate::dance_request::{DanceRequest, DanceType, RequestBody};
use crate::dance_response::ResponseBody;
use crate::staging_area::StagingArea;

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Node {
    pub source_holon: HolonReference,
    pub relationships: Option<QueryPathMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct NodeCollection {
    pub members: Vec<Node>,
    pub query_spec: Option<QueryExpression>,
}

impl NodeCollection {
    pub fn new_empty() -> Self {
        Self {
            members: Vec::new(),
            query_spec: None,
        }
    }
}

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct QueryPathMap(pub BTreeMap<RelationshipName, NodeCollection>);

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct QueryExpression {
    relationship_name: RelationshipName,
}

/// *DanceRequest:*
/// - dance_name: "add_related_holons"
/// - dance_type: Command(StagedIndex) -- references the staged holon that is the `source` of the relationship being extended
/// - request_body:
///     _TargetHolons_: specifying the RelationshipName and list of PortableReferences to the holons to add
///
/// *ResponseBody:*
/// - an Index into staged_holons that references the updated holon.
///
pub fn add_related_holons_dance(
    context: &HolonsContext,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    debug!("Entered add_related_holons_dance");

    // Match the dance_type
    match request.dance_type {
        DanceType::CommandMethod(staged_index) => {
            // Borrow a read-only reference to the CommitManager
            let staged_reference_result = {
                let commit_manager = context.commit_manager.borrow();
                debug!("Matched CommandMethod as dance_type.");
                // Convert the staged_index into a StagedReference
                commit_manager.to_staged_reference(staged_index)
            };

            // Handle the result of to_staged_reference
            match staged_reference_result {
                Ok(source_reference) => {
                    match request.body {
                        RequestBody::TargetHolons(relationship_name, holons_to_add) => {
                            // Convert Vec<PortableReference> to Vec<HolonReference> inline
                            debug!("Matched TargetHolons as RequestBody, building holon_refs_vec");

                            debug!("Got the holon_refs_vec, about to call add_related_holons");
                            // Call the add_related_holons method on StagedReference
                            source_reference.add_related_holons(
                                context,
                                relationship_name,
                                holons_to_add,
                            )?;

                            Ok(ResponseBody::Index(staged_index))
                        }
                        _ => Err(HolonError::InvalidParameter(
                            "Invalid RequestBody: expected TargetHolons, didn't get one"
                                .to_string(),
                        )),
                    }
                }
                Err(e) => Err(e),
            }
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected CommandMethod(StagedIndex), didn't get one".to_string(),
        )),
    }
}

///
/// Builds a DanceRequest for adding related holons to a source_holon.
pub fn build_add_related_holons_dance_request(
    staging_area: StagingArea,
    index: StagedIndex,
    relationship_name: RelationshipName,
    holons_to_add: Vec<HolonReference>,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_target_holons(relationship_name, holons_to_add);
    Ok(DanceRequest::new(
        MapString("add_related_holons".to_string()),
        DanceType::CommandMethod(index),
        body,
        staging_area,
    ))
}

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
    context: &HolonsContext,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    debug!("Entered query_relationships_dance");

    match request.dance_type {
        DanceType::QueryMethod(node_collection) => {
            let relationship_name = match request.body {
                RequestBody::QueryExpression(expression) => expression.relationship_name,
                _ => {
                    return Err(HolonError::InvalidParameter(
                        "Invalid RequestBody: expected QueryExpression with relationship name, \
                        didn't get one".to_string(),
                    ))
                }
            };

            let mut result_collection = NodeCollection::new_empty();

            for node in node_collection.members {
                let related_holons_rc = node
                    .source_holon
                    .get_related_holons(context, &relationship_name)?;

                let related_holons = Rc::clone(&related_holons_rc);

                let mut query_path_map = QueryPathMap::new(BTreeMap::new());

                for reference in related_holons.get_members() {
                    let mut related_collection = NodeCollection::new_empty();
                    related_collection.members.push(Node::new(reference.clone(), None));
                    query_path_map
                        .0
                        .insert(relationship_name.clone(), related_collection);
                }

                let new_node = Node::new(node.source_holon.clone(), Some(query_path_map));
                result_collection.members.push(new_node);
            }

            Ok(ResponseBody::Collection(result_collection))
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected QueryMethod, didn't get one".to_string(),
        )),
    }
}




// pub fn query_relationships_dance(
//     context: &HolonsContext,
//     request: DanceRequest,
// ) -> Result<ResponseBody, HolonError> {
//     debug!("Entered query_relationships_dance");
//
//     // Match the dance_type
//     match request.dance_type {
//         DanceType::QueryMethod(node_collection) => {
//             let relationship_name = match request.body {
//                 RequestBody::QueryExpression(expression) => expression.relationship_name,
//                 _ => {
//                     return Err(HolonError::InvalidParameter(
//                         "Invalid RequestBody: expected QueryExpression with relationship name, \
//                         didn't get one".to_string(),
//                     ))
//                 }
//             };
//             let mut result_collection = NodeCollection::new_empty();
//             for node in node_collection.members {
//                 let related_holons_map = node
//                     .source_holon
//                     .get_related_holons(context, &relationship_name)?
//                     .0;
//
//                 let mut query_path_map = QueryPathMap::new(BTreeMap::new());
//
//                 for (relationship_name, collection) in related_holons_map {
//                     let mut related_collection = NodeCollection::new_empty();
//                     for reference in collection.get_members() {
//                         related_collection.members.push(Node::new(reference, None))
//                     }
//                     query_path_map
//                         .0
//                         .insert(relationship_name, related_collection);
//                 }
//                 let new_node = Node::new(node.source_holon, Some(query_path_map));
//                 result_collection.members.push(new_node);
//             }
//             Ok(ResponseBody::Collection(result_collection))
//         }
//         _ => Err(HolonError::InvalidParameter(
//             "Invalid DanceType: expected QueryMethod, didn't get one".to_string(),
//         )),
//     }
// }

/// Builds a DanceRequest for getting related holons optionally filtered by relationship name.
pub fn build_query_relationships_dance_request(
    staging_area: StagingArea,
    node_collection: NodeCollection,
    query_expression: QueryExpression,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_query_expression(query_expression);
    Ok(DanceRequest::new(
        MapString("query_relationships".to_string()),
        DanceType::QueryMethod(node_collection),
        body,
        staging_area,
    ))
}

/// This dance creates a new holon that can be incrementally built up prior to commit.
///
/// *DanceRequest:*
/// - dance_name: "stage_new_holon"
/// - dance_type: Standalone
/// - request_body:
///     ParameterValues: specifying the initial set of properties to set in the staged_holon (if any)
///
/// *ResponseBody:*
/// - an Index into staged_holons that references the newly staged holon.
///
pub fn stage_new_holon_dance(
    context: &HolonsContext,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    debug!("== Entered staged new holon dance ==");
    // Create and stage new Holon
    let mut new_holon = Holon::new();

    // Populate parameters if available
    match request.body {
        RequestBody::None => {
            // No parameters to populate, continue
        }
        // RequestBody::ParameterValues(parameters) => {
        //     // Populate parameters into the new Holon
        //     for (property_name, base_value) in parameters.iter() {
        //         new_holon.with_property_value(property_name.clone(), base_value.clone())?;
        //     }
        // }
        RequestBody::Holon(holon) => {
            new_holon = holon;
            debug!("Request body matched holon variant");
        }
        _ => return Err(HolonError::InvalidParameter("request.body".to_string())),
    }
    debug!(
        "Response body matched successfully for holon:{:#?}",
        new_holon
    );

    // Stage the new holon
    let staged_reference = context
        .commit_manager
        .borrow_mut()
        .stage_new_holon(new_holon)?;
    // This operation will have added the staged_holon to the CommitManager's vector and returned a
    // StagedReference to it.

    Ok(ResponseBody::Index(staged_reference.holon_index))
}

/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
// pub fn build_stage_new_holon_dance_request(
//     staging_area: StagingArea,
//     properties: PropertyMap,
// ) -> Result<DanceRequest, HolonError> {
//     let body = RequestBody::new_parameter_values(properties);
//     Ok(DanceRequest::new(
//         MapString("stage_new_holon".to_string()),
//         DanceType::Standalone,
//         body,
//         staging_area,
//     ))
// }
pub fn build_stage_new_holon_dance_request(
    staging_area: StagingArea,
    holon: Holon,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_holon(holon);
    Ok(DanceRequest::new(
        MapString("stage_new_holon".to_string()),
        DanceType::Standalone,
        body,
        staging_area,
    ))
}

/// Add property values to an already staged holon
///
/// *DanceRequest:*
/// - dance_name: "with_properties"
/// - dance_type: Command(StagedIndex) -- references staged_holon to update
/// - request_body:
///     ParameterValues: specifying the set of properties to set in the staged_holon
///
/// *ResponseBody:*
/// - an Index into staged_holons that references the updated holon.
///
pub fn with_properties_dance(
    context: &HolonsContext,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // Get the staged holon
    debug!("===== ENTERED with_properties_dance");
    match request.dance_type {
        DanceType::CommandMethod(staged_index) => {
            debug!("looking for StagedHolon at index: {:#?}", staged_index);
            // Try to get a mutable reference to the staged holon referenced by its index
            let commit_manage_mut = context.commit_manager.borrow_mut();
            let staged_holon = commit_manage_mut.get_mut_holon_by_index(staged_index.clone());

            match staged_holon {
                Ok(mut holon_mut) => {
                    // Populate properties from parameters (if any)
                    match request.body {
                        RequestBody::None => {
                            // No parameters to populate, continue
                            Ok(ResponseBody::Index(staged_index))
                        }
                        RequestBody::ParameterValues(parameters) => {
                            // Populate parameters into the new Holon
                            for (property_name, base_value) in parameters {
                                holon_mut.with_property_value(
                                    property_name.clone(),
                                    base_value.clone(),
                                )?;
                            }
                            Ok(ResponseBody::Index(staged_index))
                        }
                        _ => Err(HolonError::InvalidParameter("request.body".to_string())),
                    }
                }
                Err(_) => Err(HolonError::IndexOutOfRange(
                    "Unable to borrow a mutable reference to holon at supplied staged_index"
                        .to_string(),
                )),
            }
        }
        _ => Err(HolonError::InvalidParameter(
            "Expected Command(StagedIndex) DanceType, didn't get one".to_string(),
        )),
    }
}

/// Builds a DanceRequest for adding a new property value(s) to an already staged holon.
pub fn build_with_properties_dance_request(
    staging_area: StagingArea,
    index: StagedIndex,
    properties: PropertyMap,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_parameter_values(properties);

    Ok(DanceRequest::new(
        MapString("with_properties".to_string()),
        DanceType::CommandMethod(index),
        body,
        staging_area,
    ))
}

/// Get all holons from the persistent store
///
/// *DanceRequest:*
/// - dance_name: "get_all_holons"
/// - dance_type: Standalone
/// - request_body: None
///
/// *ResponseBody:*
/// - Holons -- will be replaced by SmartCollection once supported
///
pub fn get_all_holons_dance(
    _context: &HolonsContext,
    _request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // TODO: add support for descriptor parameter
    //
    //
    debug!("Entering get_all_holons dance..");
    let query_result = Holon::get_all_holons();
    match query_result {
        Ok(holons) => {
            Ok(ResponseBody::Holons(holons))
        },
        Err(holon_error) => Err(holon_error.into()),
    }
}

/// Builds a DanceRequest for retrieving all holons from the persistent store
pub fn build_get_all_holons_dance_request(
    staging_area: StagingArea,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(
        MapString("get_all_holons".to_string()),
        DanceType::Standalone,
        body,
        staging_area,
    ))
}

/// Gets Holon from persistent store, located by HolonId (ActionHash)
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
    context: &HolonsContext,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("-------ENTERED: get_holon_by_id_dance.");
    let holon_id = match request.body {
        RequestBody::HolonId(id) => id,
        _ => {
            return Err(HolonError::InvalidParameter(
                "RequestBody variant must be HolonId".to_string(),
            ))
        }
    };
    info!("getting cache_manager from context");
    let cache_manager = context.cache_manager.borrow();

    info!("asking cache_manager to get rc_holon");
    let rc_holon = cache_manager.get_rc_holon(None, &holon_id)?;

    let holon = rc_holon.borrow().clone();
    Ok(ResponseBody::Holon(holon))
}

/// Builds a DanceRequest for retrieving holon by HolonId from the persistent store
pub fn build_get_holon_by_id_dance_request(
    staging_area: StagingArea,
    holon_id: HolonId,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::HolonId(holon_id);
    Ok(DanceRequest::new(
        MapString("get_holon_by_id".to_string()),
        DanceType::Standalone,
        body,
        staging_area,
    ))
}

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
    context: &HolonsContext,
    _request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    debug!("Entered: commit_dance");
    let commit_response = CommitManager::commit(context);

    match commit_response.status {
        Complete => Ok(ResponseBody::Holons(commit_response.saved_holons)),
        Incomplete => {
            let completion_message = format!(
                "{} of {:?} were successfully committed",
                commit_response.saved_holons.len(),
                commit_response.commits_attempted.0,
            );
            // TODO: Why turn INCOMPLETE into an Error? Shouldn't we just pass CommitResponse to client?
            Err(HolonError::CommitFailure(completion_message.to_string()))
        }
    }
}

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_commit_dance_request(staging_area: StagingArea) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(
        MapString("commit".to_string()),
        DanceType::Standalone,
        body,
        staging_area,
    ))
}
/// Abandon staged changes
///
/// *DanceRequest:*
/// - dance_name: "abandon_staged_changes"
/// - dance_type: Command(StagedIndex) -- references the staged holon whose changes are being abandoned
/// - request_body: None
///   
///
/// *ResponseBody:*
/// - an Index into staged_holons that references the updated holon.
///
pub fn abandon_staged_changes_dance(
    context: &HolonsContext,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // Get the staged holon
    debug!("Entered: abandon_staged_changes_dance");
    match request.dance_type {
        DanceType::CommandMethod(staged_index) => {
            debug!("trying to borrow_mut commit_manager");
            // Try to get a mutable reference to the staged holon referenced by its index
            let commit_manager_mut = context.commit_manager.borrow_mut();
            debug!("commit_manager borrowed_mut");
            let staged_holon = commit_manager_mut.get_mut_holon_by_index(staged_index.clone());
            //debug!("Result of borrow_mut on the staged holon  {:#?}", staged_holon.clone());

            match staged_holon {
                Ok(mut holon_mut) => {
                    holon_mut.abandon_staged_changes()?;
                    Ok(ResponseBody::Index(staged_index))
                }
                Err(_) => Err(HolonError::IndexOutOfRange(
                    "Unable to borrow a mutable reference to holon at supplied staged_index"
                        .to_string(),
                )),
            }
        }
        _ => Err(HolonError::InvalidParameter(
            "Expected Command(StagedIndex) DanceType, didn't get one".to_string(),
        )),
    }
}

///
/// Builds a DanceRequest for abandoning changes to a staged Holon.
pub fn build_abandon_staged_changes_dance_request(
    staging_area: StagingArea,
    index: StagedIndex,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(
        MapString("abandon_staged_changes".to_string()),
        DanceType::CommandMethod(index),
        body,
        staging_area,
    ))
}

//
// #[hdk_extern]
// pub fn with_property_value(input: WithPropertyInput) -> ExternResult<Holon> {
//     let mut holon = input.holon.clone();
//     holon.with_property_value(
//         input.property_name.clone(),
//         input.value.clone());
//     Ok(holon)
// }
// #[hdk_extern]
// pub fn get_holon(
//     id: HolonId,
// ) -> ExternResult<Option<Holon>> {
//        match Holon::get_holon(id) {
//         Ok(result) => Ok(result),
//         Err(holon_error) => {
//             Err(holon_error.into())
//         }
//     }
// }
//
// #[hdk_extern]
// pub fn commit(input: Holon) -> ExternResult<Holon> {
//     let holon = input.clone();
//     // // quick exit to test error return
//     // return Err(HolonError::NotImplemented("load_core_schema_aoi".to_string()).into());
//     match holon.commit() {
//         Ok(result)=> Ok(result.clone()),
//         Err(holon_error) => {
//             Err(holon_error.into())
//         }
//     }
//
// }
//
// #[hdk_extern]
// pub fn get_all_holons(
//    _: (),
// ) -> ExternResult<Vec<Holon>> {
//     match Holon::get_all_holons() {
//         Ok(result) => Ok(result),
//         Err(holon_error) => {
//             Err(holon_error.into())
//         }
//     }
//
// }
// #[hdk_extern]
// pub fn delete_holon(
//     target_holon_id: ActionHash,
// ) -> ExternResult<ActionHash> {
//     match delete_holon_node(target_holon_id) {
//         Ok(result) => Ok(result),
//         Err(holon_error) => {
//             Err(holon_error.into())
//         }
//     }
// }

/*
#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateHolonNodeInput {
    pub original_holon_hash: ActionHash,
    pub previous_holon_hash: ActionHash,
    pub updated_holon: HolonNode,
}
#[hdk_extern]
pub fn update_holon(input: UpdateHolonNodeInput) -> ExternResult<Record> {
    let updated_holon_hash = update_entry(
        input.previous_holon_hash.clone(),
        &input.updated_holon,
    )?;
    create_link(
        input.original_holon_hash.clone(),
        updated_holon_hash.clone(),
        LinkTypes::HolonNodeUpdates,
        (),
    )?;
    let record = get(updated_holon_hash.clone(), GetOptions::default())?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Could not find the newly updated HolonNode"))
            ),
        )?;
    Ok(record)
}

 */
