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

use hdk::prelude::*;
use holons_core::core_shared_objects::{
    commit_api, delete_holon_api, stage_new_from_clone_api, stage_new_holon_api,
    stage_new_version_api, CommitRequestStatus, Holon, HolonError, RelationshipName,
};
use holons_core::{
    HolonReference, HolonWritable, HolonsContextBehavior, SmartReference, StagedReference,
};
use holons_guest::query_layer::{evaluate_query, NodeCollection, QueryExpression};
use shared_types_holon::{HolonId, LocalId};
use shared_types_holon::{MapString, PropertyMap};
use std::sync::Arc;

use crate::dance_request::{DanceRequest, DanceType, RequestBody};
use crate::dance_response::ResponseBody;
use crate::session_state::SessionState;

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
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered add_related_holons_dance");

    // Match the dance_type
    match request.dance_type {
        DanceType::CommandMethod(staged_reference) => {
            // // Borrow a read-only reference to the CommitManager
            // let staged_reference_result = {
            //     let space_manager = context.get_space_manager();
            //     debug!("Matched CommandMethod as dance_type.");
            //     // Convert the staged_index into a StagedReference
            //     space_manager.to_validated_staged_reference(staged_index)
            // };

            // Handle the result of to_staged_reference
            // match staged_reference {
            //     Ok(source_reference) => {
            match request.body {
                RequestBody::TargetHolons(relationship_name, holons_to_add) => {
                    // Convert Vec<PortableReference> to Vec<HolonReference> inline
                    debug!("Matched TargetHolons as RequestBody, building Vec<HolonReference>");

                    debug!("Got the Vec<HolonReference>, about to call add_related_holons");
                    // Call the add_related_holons method on StagedReference
                    staged_reference.add_related_holons(
                        context,
                        relationship_name,
                        holons_to_add,
                    )?;

                    Ok(ResponseBody::StagedReference(staged_reference))
                }
                _ => Err(HolonError::InvalidParameter(
                    "Invalid RequestBody: expected TargetHolons, didn't get one".to_string(),
                )),
            }
            //     }
            //     Err(e) => Err(e),
            // }
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected CommandMethod(StagedReference), didn't get one"
                .to_string(),
        )),
    }
}
///
/// Builds a DanceRequest for adding related holons to a source_holon.
pub fn build_add_related_holons_dance_request(
    session_state: &SessionState,
    staged_reference: StagedReference,
    relationship_name: RelationshipName,
    holons_to_add: Vec<HolonReference>,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_target_holons(relationship_name, holons_to_add);
    Ok(DanceRequest::new(
        MapString("add_related_holons".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        session_state.clone(),
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
    context: &dyn HolonsContextBehavior,
    _request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered commit_dance");
    let commit_response = commit_api(context)?;

    match commit_response.status {
        CommitRequestStatus::Complete => Ok(ResponseBody::Holons(commit_response.saved_holons)),
        CommitRequestStatus::Incomplete => {
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
pub fn build_commit_dance_request(
    session_state: &SessionState,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(
        MapString("commit".to_string()),
        DanceType::Standalone,
        body,
        session_state.clone(),
    ))
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
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered delete_holon dance");
    match request.dance_type {
        DanceType::DeleteMethod(holon_id) => {
            // Call the new `delete_holon_api` function
            delete_holon_api(context, holon_id).map(|_| ResponseBody::None)
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected DeleteMethod(HolonId), didn't get one".to_string(),
        )),
    }
}

/// Builds a DanceRequest for deleting a local Holon from the persistent store
pub fn build_delete_holon_dance_request(
    session_state: &SessionState,
    holon_to_delete: LocalId,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(
        MapString("delete_holon".to_string()),
        DanceType::DeleteMethod(holon_to_delete),
        body,
        session_state.clone(),
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
    _context: &dyn HolonsContextBehavior,
    _request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // TODO: add support for descriptor parameter
    //
    //
    info!("----- Entered get_all_holons dance");
    let query_result = Holon::get_all_holons();
    match query_result {
        Ok(holons) => Ok(ResponseBody::Holons(holons)),
        Err(holon_error) => Err(holon_error.into()),
    }
}

/// Builds a DanceRequest for retrieving all holons from the persistent store
pub fn build_get_all_holons_dance_request(
    session_state: &SessionState,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(
        MapString("get_all_holons".to_string()),
        DanceType::Standalone,
        body,
        session_state.clone(),
    ))
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
    context: &dyn HolonsContextBehavior,
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
    debug!("getting space_manager from context");
    let space_manager = context.get_space_manager();
    let holon_service = space_manager.get_holon_service();

    debug!("asking space_manager to get rc_holon");
    let holon = holon_service.fetch_holon(&holon_id)?;

    let holon = holon.clone();
    Ok(ResponseBody::Holon(holon))
}

/// Builds a DanceRequest for retrieving holon by HolonId from the persistent store
pub fn build_get_holon_by_id_dance_request(
    session_state: &SessionState,
    holon_id: HolonId,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::HolonId(holon_id);
    Ok(DanceRequest::new(
        MapString("get_holon_by_id".to_string()),
        DanceType::Standalone,
        body,
        session_state.clone(),
    ))
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
    context: &dyn HolonsContextBehavior,
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

            let result_collection = evaluate_query(node_collection, context, relationship_name)?;
            Ok(ResponseBody::Collection(result_collection))
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected QueryMethod, didn't get one".to_string(),
        )),
    }
}

///
/// Builds a DanceRequest for getting related holons optionally filtered by relationship name.
pub fn build_query_relationships_dance_request(
    session_state: &SessionState,
    node_collection: NodeCollection,
    query_expression: QueryExpression,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_query_expression(query_expression);
    Ok(DanceRequest::new(
        MapString("query_relationships".to_string()),
        DanceType::QueryMethod(node_collection),
        body,
        session_state.clone(),
    ))
}
/// Remove related Holons
///
/// *DanceRequest:*
/// - dance_name: "remove_related_holons"
/// - dance_type: CommandMethod(StagedIndex) -- identifies the holon that is the `source` of the relationship being navigated
/// - request_body:
///     TargetHolons(RelationshipName, Vec<HolonReference>),
///
/// *ResponseBody:*
/// - StagedReference(StagedReference) -- index for the staged_holon for which related holons were removed
///
///
pub fn remove_related_holons_dance(
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("Entered remove_related_holons_dance");

    // Match the dance_type
    match request.dance_type {
        DanceType::CommandMethod(staged_reference) => {
            // // Borrow a read-only reference to the CommitManager
            // let staged_reference_result = {
            //     let space_manager = context.get_space_manager();
            //     debug!("Matched CommandMethod as dance_type.");
            //     // Convert the staged_index into a StagedReference
            //     space_manager.to_validated_staged_reference(staged_reference)
            // };
            //
            // // Handle the result of to_staged_reference
            // match staged_reference_result {
            //     Ok(source_reference) => {
            match request.body {
                RequestBody::TargetHolons(relationship_name, holons_to_remove) => {
                    // Convert Vec<PortableReference> to Vec<HolonReference> inline
                    debug!("Matched TargetHolons as RequestBody, building holon_refs_vec");

                    debug!("Got the holon_refs_vec, about to call remove_related_holons");
                    staged_reference.remove_related_holons(
                        context,
                        &relationship_name,
                        holons_to_remove,
                    )?;

                    Ok(ResponseBody::StagedReference(staged_reference))
                }
                _ => Err(HolonError::InvalidParameter(
                    "Invalid RequestBody: expected TargetHolons, didn't get one".to_string(),
                )),
            }
            //     }
            //     Err(e) => Err(e),
            // }
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected CommandMethod(StagedIndex), didn't get one".to_string(),
        )),
    }
}

/// Builds a DanceRequest for removing related holons to a source_holon.
pub fn build_remove_related_holons_dance_request(
    session_state: &SessionState,
    staged_reference: StagedReference,
    relationship_name: RelationshipName,
    holons_to_remove: Vec<HolonReference>,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_target_holons(relationship_name, holons_to_remove);
    Ok(DanceRequest::new(
        MapString("remove_related_holons".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        session_state.clone(),
    ))
}
/// Stages a new Holon by cloning an existing Holon, without retaining lineage to the Holon its cloned from.
///
/// *DanceRequest:*
/// - dance_name: "stage_new_from_clone"
/// - dance_type: CloneMethod(HolonReference)
/// - request_body: None
///
///
/// *ResponseBody:*
/// StagedReference(StagedReference), // a reference to the newly staged holon
///
pub fn stage_new_from_clone_dance(
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered stage_new_from_clone dance");

    let holon_reference = match request.dance_type {
        DanceType::CloneMethod(holon_reference) => holon_reference,
        _ => {
            return Err(HolonError::InvalidParameter(
                "Invalid DanceType: expected CloneMethod, didn't get one".to_string(),
            ));
        }
    };

    let staged_reference = stage_new_from_clone_api(context, holon_reference)?;

    Ok(ResponseBody::StagedReference(staged_reference))
}
///
/// Builds a dance request for staging a new cloned Holon
pub fn build_stage_new_from_clone_dance_request(
    session_state: &SessionState,
    holon_reference: HolonReference,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        MapString("stage_new_from_clone".to_string()),
        DanceType::CloneMethod(holon_reference),
        RequestBody::None,
        session_state.clone(),
    ))
}

/// This dance creates a new version of an existing holon by cloning the existing holon, adding
/// the clone to the StagingArea and resetting its PREDECESSOR relationship to reference the
/// holon it was cloned from. The cloned holon can then be incrementally built up prior to commit.
///
/// *DanceRequest:*
/// - dance_name: "stage_new_version"
/// - dance_type: Standalone
/// - request_body:
///     ParameterValues: specifying the initial set of properties to set in the staged_holon (if any)
///
/// *ResponseBody:*
/// - an Index into staged_holons that references the newly staged holon.
///
pub fn stage_new_holon_dance(
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered stage new holon dance");
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
    debug!("Response body matched successfully for holon:{:#?}", new_holon);

    // Stage the new holon
    let staged_reference = stage_new_holon_api(context, new_holon)?;
    // This operation will have added the staged_holon to the CommitManager's vector and returned a
    // StagedReference to it.

    Ok(ResponseBody::StagedReference(staged_reference))
}
///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
// pub fn build_stage_new_holon_dance_request(
//     session_state: &SessionState,
//     properties: PropertyMap,
// ) -> Result<DanceRequest, HolonError> {
//     let body = RequestBody::new_parameter_values(properties);
//     Ok(DanceRequest::new(
//         MapString("stage_new_holon".to_string()),
//         DanceType::Standalone,
//         body,
//         session_state.clone(),
//     ))
// }
pub fn build_stage_new_holon_dance_request(
    session_state: &SessionState,
    holon: Holon,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_holon(holon);
    Ok(DanceRequest::new(
        MapString("stage_new_holon".to_string()),
        DanceType::Standalone,
        body,
        session_state.clone(),
    ))
}

/// Stages a new version of a Holon by cloning an existing Holon, retaining lineage to the Holon its cloned from.
/// This operation is only allowed for smart references.
///
/// *DanceRequest:*
/// - dance_name: "stage_new_version"
/// - dance_type: NewVersionMethod(HolonId)
/// - request_body: None
///
///
/// *ResponseBody:*
/// StagedReference(StagedReference), // a reference to the newly staged holon
///
pub fn stage_new_version_dance(
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered stage_new_version dance ==");

    let smart_reference = match request.dance_type {
        DanceType::NewVersionMethod(holon_id) => SmartReference::new(holon_id, None), // TODO: handle getting smart_prop_vals
        _ => {
            return Err(HolonError::InvalidParameter(
                "Invalid DanceType: expected CloneMethod, didn't get one".to_string(),
            ));
        }
    };

    let staged_reference = stage_new_version_api(context, smart_reference)?;

    Ok(ResponseBody::StagedReference(staged_reference))
}
///
/// Builds a dance request for staging a new cloned Holon
pub fn build_stage_new_version_dance_request(
    session_state: &SessionState,
    holon_id: HolonId,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        MapString("stage_new_version".to_string()),
        DanceType::NewVersionMethod(holon_id),
        RequestBody::None,
        session_state.clone(),
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
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // Get the staged holon
    info!("----- Entered with_properties_dance");
    match request.dance_type {
        DanceType::CommandMethod(staged_reference) => {
            // debug!("looking for StagedHolon at index: {:#?}", staged_reference.holon_index);
            // // Try to get a mutable reference to the staged holon referenced by its index
            // let space_manager = match context.get_space_manager() {
            //     Ok(space_manager) => space_manager,
            //     Err(borrow_error) => {
            //         error!(
            //             "Failed to borrow commit_manager, it is already borrowed mutably: {:?}",
            //             borrow_error
            //         );
            //         return Err(HolonError::FailedToBorrow(format!("{:?}", borrow_error)));
            //     }
            // };
            // //let staged_holon = space_manager.get_mut_holon_by_index(staged_index.clone());
            // let holon = space_manager.get_holon_by_index(staged_index.clone())?;
            // let staged_holon = holon.try_borrow_mut().map_err(|e| {
            //     HolonError::FailedToBorrow(format!("Unable to borrow holon immutably: {}", e))
            // });

            // match staged_holon {
            //     Ok(mut holon_mut) => {
            // Populate properties from parameters (if any)
            match request.body {
                RequestBody::None => {
                    // No parameters to populate, continue
                    Ok(ResponseBody::StagedReference(staged_reference))
                }
                RequestBody::ParameterValues(parameters) => {
                    // Populate parameters into the new Holon
                    for (property_name, base_value) in parameters {
                        staged_reference.with_property_value(
                            context,
                            property_name.clone(),
                            base_value.clone(),
                        )?;
                    }
                    Ok(ResponseBody::StagedReference(staged_reference))
                }
                _ => Err(HolonError::InvalidParameter("request.body".to_string())),
            }
            //     }
            //     Err(_) => Err(HolonError::IndexOutOfRange(
            //         "Unable to borrow a mutable reference to holon at supplied staged_index"
            //             .to_string(),
            //     )),
            // }
        }
        _ => Err(HolonError::InvalidParameter(
            "Expected Command(StagedReference) DanceType, didn't get one".to_string(),
        )),
    }
}
///
/// Builds a DanceRequest for adding a new property value(s) to an already staged holon.
pub fn build_with_properties_dance_request(
    session_state: &SessionState,
    staged_reference: StagedReference,
    properties: PropertyMap,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_parameter_values(properties);

    Ok(DanceRequest::new(
        MapString("with_properties".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        session_state.clone(),
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
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // Get the staged holon
    info!("----- Entered abandon_staged_changes_dance");
    match request.dance_type {
        DanceType::CommandMethod(mut staged_reference) => {
            staged_reference.abandon_staged_changes(context)?;
            Ok(ResponseBody::StagedReference(staged_reference))
        }
        _ => Err(HolonError::InvalidParameter(
            "Expected Command(StagedReference) DanceType, didn't get one".to_string(),
        )),
    }
}

///
/// Builds a DanceRequest for abandoning changes to a staged Holon.
pub fn build_abandon_staged_changes_dance_request(
    session_state: &SessionState,
    staged_reference: StagedReference,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(
        MapString("abandon_staged_changes".to_string()),
        DanceType::CommandMethod(staged_reference),
        body,
        session_state.clone(),
    ))
}
