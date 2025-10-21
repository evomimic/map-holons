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
//!   details and vice-versa.
//! - Error mapping to `DanceResponse` status codes is handled by the dancer/dispatch layer;
//!   adapters return `Result<ResponseBody, HolonError>`.

use tracing::{debug, info};

use crate::reference_layer::TransientReference;
use crate::{
    core_shared_objects::{
        commit, delete_holon, stage_new_from_clone, stage_new_holon, stage_new_version,
        CommitRequestStatus,
    },
    dances::{
        dance_request::{DanceType, RequestBody},
        dance_response::ResponseBody,
        DanceRequest,
    },
    load_holons,
    query_layer::evaluate_query,
    reference_layer::{
        holon_operations_api::get_all_holons, holon_operations_api::new_holon,
        writable_holon::WritableHolon, HolonReference, HolonsContextBehavior, SmartReference,
    },
};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, PropertyName};

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
    let commit_response = commit(context)?;

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
            delete_holon(context, holon_id).map(|_| ResponseBody::None)
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
    let holon = holon_service.fetch_holon_internal(&holon_id)?;

    let holon = holon.clone();
    Ok(ResponseBody::Holon(holon))
}

/// Executes the `"load_holons"` dance (guest side).
///
/// This adapter validates the request and delegates Holon import to the
/// guest `HolonServiceApi` implementation, which calls the loader controller.
///
/// *DanceRequest:*
/// - dance_name: **"load_holons"**
/// - dance_type: **Standalone**
/// - request_body: **TransientReference(…HolonLoaderBundle…)**
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
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered load holons dance");
    // Validate dance type
    if request.dance_type != DanceType::Standalone {
        return Err(HolonError::InvalidParameter(
            "Invalid DanceType: expected Standalone".to_string(),
        ));
    }

    // Extract bundle reference
    let bundle_reference: TransientReference = match request.body {
        RequestBody::TransientReference(transient_reference) => transient_reference,
        _ => {
            return Err(HolonError::InvalidParameter(
                "Invalid RequestBody: expected TransientReference (HolonLoaderBundle)".to_string(),
            ))
        }
    };

    // Delegate to the public ops API (which calls the *_internal service impl)
    let response_reference = load_holons(context, bundle_reference)?;

    // Wrap transient response holon
    Ok(ResponseBody::HolonReference(HolonReference::Transient(response_reference)))
}

pub fn new_holon_dance(
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    info!("----- Entered new holon dance");
    if request.dance_type != DanceType::Standalone {
        return Err(HolonError::InvalidParameter("Invalid DanceType: expected Standalone".into()));
    }

    // Optional key via ParameterValues; allow None for keyless creation.
    let key_option: Option<MapString> = match &request.body {
        RequestBody::ParameterValues(map) => {
            let property_name = PropertyName(MapString("key".into()));
            match map.get(&property_name) {
                Some(BaseValue::StringValue(key)) => Some(key.clone()),
                Some(_) => return Err(HolonError::InvalidParameter("key must be a string".into())),
                None => None,
            }
        }
        RequestBody::None => None,
        _ => {
            return Err(HolonError::InvalidParameter(
                "Invalid RequestBody: expected None or ParameterValues".into(),
            ))
        }
    };

    // Delegate to the public API; it will route to the *_internal impl for this env.
    let response_reference = new_holon(context, key_option)?;
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

    let staged_reference =
        stage_new_from_clone(context, original_holon, MapString(Into::<String>::into(&new_key)))?;

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
            stage_new_holon(context, reference)?
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
        DanceType::NewVersionMethod(holon_id) => SmartReference::new_from_id(holon_id), // TODO: handle getting smart_prop_vals
        _ => {
            return Err(HolonError::InvalidParameter(
                "Invalid DanceType: expected CloneMethod, didn't get one".to_string(),
            ));
        }
    };

    let staged_reference = stage_new_version(context, smart_reference)?;

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
