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

use tracing::{debug, info};

use crate::{
    core_shared_objects::{
        commit_api, delete_holon_api, stage_new_from_clone_api, stage_new_holon_api,
        stage_new_version_api, CommitRequestStatus,
    },
    dances::{
        dance_request::{DanceType, RequestBody},
        dance_response::ResponseBody,
        DanceRequest,
    },
    query_layer::evaluate_query,
    reference_layer::{
        holon_operations_api::get_all_holons, writable_holon::WriteableHolon, HolonReference,
        HolonsContextBehavior, SmartReference,
    },
};
use base_types::MapString;
use core_types::HolonError;
use integrity_core_types::PropertyName;

/// Abandon staged changes
///
/// *DanceRequest:*
/// - dance_name: "abandon_staged_changes"
/// - dance_type: Command(HolonReference) -- references the staged holon whose changes are being abandoned
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
        DanceType::CommandMethod(holon_reference) => {
            match &holon_reference {
                HolonReference::Staged(staged_reference) => {
                    staged_reference.abandon_staged_changes(context)?;
                }
                _ => {
                    return Err(HolonError::InvalidHolonReference(
                        "Can only abandon staged changes on a StagedReference".to_string(),
                    ))
                }
            }

            Ok(ResponseBody::HolonReference(holon_reference))
        }
        _ => Err(HolonError::InvalidParameter(
            "Expected Command(StagedReference) DanceType, didn't get one".to_string(),
        )),
    }
}

/// *DanceRequest:*
/// - dance_name: "add_related_holons"
/// - dance_type: CommandMethod(HolonReference) -- references the holon that is the `source` of the relationship being extended
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
        DanceType::CommandMethod(holon_reference) => {
            match request.body {
                RequestBody::TargetHolons(relationship_name, holons_to_add) => {
                    debug!("Got the Vec<HolonReference>, about to call add_related_holons");
                    match &holon_reference {
                        HolonReference::Transient(transient_reference) => {
                            // Call the add_related_holons method on HolonReference
                            transient_reference.add_related_holons(
                                context,
                                relationship_name,
                                holons_to_add,
                            )?;
                        }
                        HolonReference::Staged(staged_reference) => {
                            staged_reference.add_related_holons(
                                context,
                                relationship_name,
                                holons_to_add,
                            )?;
                        }
                        HolonReference::Smart(_) => {
                            return Err(HolonError::InvalidHolonReference(
                                "Cannot add relationships to a SmartReference, which is immutable"
                                    .to_string(),
                            ))
                        }
                    }
                    Ok(ResponseBody::HolonReference(holon_reference))
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
    context: &dyn HolonsContextBehavior,
    _request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // TODO: add support for descriptor parameter
    //
    //
    info!("----- Entered get_all_holons dance ----");
    Ok(ResponseBody::HolonCollection(get_all_holons(context)?))
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
            Ok(ResponseBody::NodeCollection(result_collection))
        }
        _ => Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected QueryMethod, didn't get one".to_string(),
        )),
    }
}

/// Remove related Holons
///
/// *DanceRequest:*
/// - dance_name: "remove_related_holons"
/// - dance_type: CommandMethod(HolonReference) -- identifies the holon that is the `source` of the relationship being navigated
/// - request_body:
///     TargetHolons(RelationshipName, Vec<HolonReference>),
///
/// *ResponseBody:*
/// - HolonReference(HolonReference) -- index for the staged_holon for which related holons were removed
///
///
pub fn remove_related_holons_dance(
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("Entered remove_related_holons_dance");

    // Match the dance_type
    match request.dance_type {
        DanceType::CommandMethod(holon_reference) => {
            match request.body {
                RequestBody::TargetHolons(relationship_name, holons_to_remove) => {
                    // Convert Vec<PortableReference> to Vec<HolonReference> inline
                    debug!("Matched TargetHolons as RequestBody, building holon_refs_vec");

                    debug!("Got the holon_refs_vec, about to call remove_related_holons");
                    match &holon_reference {
                        HolonReference::Transient(transient_reference) => {
                            // Call the remove_related_holons method on HolonReference
                            transient_reference.remove_related_holons(
                                context,
                                relationship_name,
                                holons_to_remove,
                            )?;
                        }
                        HolonReference::Staged(staged_reference) => {
                            staged_reference.remove_related_holons(
                                context,
                                relationship_name,
                                holons_to_remove,
                            )?;
                        }
                        HolonReference::Smart(_) => return Err(HolonError::InvalidHolonReference(
                            "Cannot remove relationships from a SmartReference, which is immutable"
                                .to_string(),
                        )),
                    }
                    Ok(ResponseBody::HolonReference(holon_reference))
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

/// Stages a new Holon by cloning an existing Holon, without retaining lineage to the Holon its cloned from.
///
/// *DanceRequest:*
/// - dance_name: "stage_new_from_clone"
/// - dance_type: CloneMethod(HolonReference)
/// - request_body: ParemeterValues(PropertyMap)
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

    let original_holon = match request.dance_type {
        DanceType::CloneMethod(holon_reference) => holon_reference,
        _ => {
            return Err(HolonError::InvalidParameter(
                "Invalid DanceType: expected CloneMethod, didn't get one".to_string(),
            ));
        }
    };

    let property_map = match request.body {
        RequestBody::ParameterValues(property_map) => property_map,
        _ => {
            return Err(HolonError::InvalidParameter(
                "Invalid DanceType: expected CloneMethod, didn't get one".to_string(),
            ));
        }
    };

    let new_key = property_map
        .get(&PropertyName(MapString("key".to_string())))
        .ok_or(HolonError::InvalidParameter(
            "ParameterValues PropertyMap must have a key".to_string(),
        ))?
        .clone();

    let staged_reference = stage_new_from_clone_api(
        context,
        original_holon,
        MapString(Into::<String>::into(&new_key)),
    )?;

    Ok(ResponseBody::HolonReference(HolonReference::Staged(staged_reference)))
}

/// This dance stages a new holon in the holon space.
///
/// This function creates a new holon in the staging area without any lineage
/// relationship to an existing holon. Use this function for creating entirely
/// new holons that are not tied to any predecessor.
///
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
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered stage new holon dance");

    let staged_reference = {
        if let RequestBody::TransientReference(reference) = request.body {
            // Stage the new holon
            stage_new_holon_api(context, reference)?
            // This operation will have added the staged_holon to the CommitManager's vector and returned a
            // StagedReference to it.
        } else {
            return Err(HolonError::InvalidParameter("request.body".to_string()));
        }
    };

    Ok(ResponseBody::HolonReference(HolonReference::Staged(staged_reference)))
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

    Ok(ResponseBody::HolonReference(HolonReference::Staged(staged_reference)))
}

/// Add property values to an already staged holon
///
/// *DanceRequest:*
/// - dance_name: "with_properties"
/// - dance_type: Command(StagedReference) -- references staged_holon to update
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
        DanceType::CommandMethod(holon_reference) => {
            match request.body {
                RequestBody::ParameterValues(parameters) => {
                    // Populate parameters into the new Holon
                    for (property_name, base_value) in parameters {
                        holon_reference.with_property_value(
                            context,
                            property_name.clone(),
                            base_value.clone(),
                        )?;
                    }
                    Ok(ResponseBody::HolonReference(holon_reference))
                }
                _ => Err(HolonError::InvalidParameter("request.body".to_string())),
            }
        }
        _ => Err(HolonError::InvalidParameter(
            "Expected Command(StagedReference) DanceType, didn't get one".to_string(),
        )),
    }
}
