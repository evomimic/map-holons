/// This file defines the DancesAdaptors offered by the holons zome.
/// TODO: Move these adaptors to their own zome
///
/// For each Dance, this file defines:
/// - a `build_` function as a helper function for creating DanceRequests for that Dance from
/// native parameters.
/// - a function that performs the dance
///
///
/// /// As a dance adaptor, this function wraps (and insulates) Dancer from native functionality
/// and insulates the native function from any dependency on Dances. In general, this means:
/// 1.  Extracting any required input parameters from the DanceRequest's request_body
/// 2.  Invoking the native function
/// 3.  Creating a DanceResponse based on the results returned by the native function. This includes,
/// mapping any errors into an appropriate ResponseStatus and returning results in the body.
use hdk::prelude::*;

use holons::commit_manager::CommitRequestStatus::*;
use holons::commit_manager::StagedIndex;
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use shared_types_holon::HolonId;
use shared_types_holon::{MapInteger, MapString, PropertyMap};

use crate::dance_request::{DanceRequest, DanceType, RequestBody};
use crate::dance_response::ResponseBody;
use crate::staging_area::StagingArea;

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
    // Create and stage new Holon
    let mut new_holon = Holon::new();

    // Populate parameters if available
    match request.body {
        RequestBody::None => {
            // No parameters to populate, continue
        }
        RequestBody::ParameterValues(parameters) => {
            // Populate parameters into the new Holon
            for (property_name, base_value) in parameters.iter() {
                new_holon.with_property_value(property_name.clone(), base_value.clone());
            }
        }
        _ => return Err(HolonError::InvalidParameter("request.body".to_string())),
    }

    // Stage the new holon
    let staged_reference = context
        .commit_manager
        .borrow_mut()
        .stage_new_holon(new_holon);
    // This operation will have added the staged_holon to the CommitManager's vector and returned a
    // StagedReference to it.

    // Convert the holon_index in the StagedReference into a MapInteger
    // and then return it in the response body
    let index = MapInteger(
        staged_reference
            .holon_index
            .try_into()
            .expect("Conversion failed"),
    );
    Ok(ResponseBody::Index(index))
}

/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_stage_new_holon_dance_request(
    staging_area: StagingArea,
    properties: PropertyMap,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new_parameter_values(properties);
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
    match request.dance_type {
        DanceType::Command(staged_index) => {
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
                                holon_mut
                                    .with_property_value(property_name.clone(), base_value.clone());
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
        DanceType::Command(index),
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
    let query_result = Holon::get_all_holons();
    match query_result {
        Ok(holons) => Ok(ResponseBody::Holons(holons)),
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
    _context: &HolonsContext,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    let holon_id = match request.body {
        RequestBody::HolonId(id) => id,
        _ => {
            return Err(HolonError::InvalidParameter(
                "RequestBody variant must be HolonId".to_string(),
            ))
        }
    };
    let query = Holon::get_holon(holon_id)?;
    if let Some(holon) = query {
        Ok(ResponseBody::Holon(holon))
    } else {
        Err(HolonError::HolonNotFound(
            "get_holon returned None for given HolonId".to_string(),
        ))
    }
}

/// Builds a DanceRequest for retrieving holon by HolonId from the persistent store
pub fn build_get_holon_by_id_dance_request(
    staging_area: StagingArea,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
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
    let commit_response = context.commit_manager.borrow_mut().commit(context);

    match commit_response.status {
        Complete => Ok(ResponseBody::Holons(commit_response.saved_holons)),
        Incomplete => {
            let completion_message = format!(
                "{} of {:?} were successfully committed",
                commit_response.saved_holons.len(),
                commit_response.commits_attempted,
            );
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
