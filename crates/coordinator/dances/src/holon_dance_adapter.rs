/// This file defines the DancesAdaptors offered by the holons zome. For each Dance, this file
/// also defines a `build_` function as a helper function for creating DanceRequests for that
/// Dance from native parameters.
/// They are bundled in the dances zome for now, but in the future they probably should be moved
/// into their own zome.
use hdk::prelude::*;

use holons::commit_manager::CommitRequestStatus::{Error, Success};
use holons::commit_manager::StagedIndex;
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use shared_types_holon::{MapInteger, MapString, PropertyMap};

use crate::dance_request::{DanceRequest, DanceType, RequestBody};
use crate::dance_response::ResponseBody;
use crate::staging_area::StagingArea;

/// Create a new holon that can be incrementally built up prior to commit.
/// ParameterValues supplied in the body of the request (if any) are used to set properties on
/// holon.
/// As a dance adaptor, this function wraps (and insulates) Dancer from native functionality
/// and insulates the native function from any dependency on Dances. In general, this means:
/// 1.  Extracting any required input parameters from the DanceRequest's request_body
/// 2.  Invoking the native function
/// 3.  Creating a DanceResponse based on the results returned by the native function. This includes,
/// mapping any errors into an appropriate ResponseStatus and returning results in the body.
///
///
pub fn stage_new_holon_dance(context: &HolonsContext, request: DanceRequest) -> Result<Option<ResponseBody>, HolonError> {
    // Create a new Holon
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
    let staged_reference = context.commit_manager.borrow_mut().stage_new_holon(new_holon);
    // This operation will have added the staged_holon to the CommitManager's vector and returned a
    // StagedReference to it.

    // Convert the holon_index in the StagedReference into a MapInteger
    // and then return it in the response body
    let index = MapInteger(staged_reference.holon_index.try_into().expect("Conversion failed"));
    Ok(Some(ResponseBody::Index(index)))
}

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_stage_new_holon_dance_request(staging_area: StagingArea, properties:PropertyMap)->Result<DanceRequest, HolonError> {
    let body = RequestBody::new_parameter_values(properties);
    Ok(DanceRequest::new(MapString("stage_new_holon".to_string()), DanceType::Standalone,body, staging_area))
    }

/// Add property values to an already staged holon
/// ParameterValues supplied in the body of the request are used to set properties on the holon
///
/// As a dance adaptor, this function wraps (and insulates) Dancer from native functionality
/// and insulates the native function from any dependency on Dances. In general, this means:
/// 1.  Extracting any required input parameters from the DanceRequest's request_body
/// 2.  Invoking the native function
/// 3.  Creating a DanceResponse based on the results returned by the native function. This includes,
/// mapping any errors into an appropriate ResponseStatus and returning results in the body.
///
///
pub fn with_properties_dance(context: &HolonsContext, request: DanceRequest) -> Result<Option<ResponseBody>, HolonError> {
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
                            Ok(Some(ResponseBody::Index(staged_index)))
                        },
                        RequestBody::ParameterValues(parameters) => {
                            // Populate parameters into the new Holon
                            for (property_name, base_value) in parameters {
                                holon_mut.with_property_value(property_name.clone(), base_value.clone());
                            }
                            Ok(Some(ResponseBody::Index(staged_index)))
                        },
                        _ => Err(HolonError::InvalidParameter("request.body".to_string())),
                    }
                },
                Err(_) => Err(HolonError::IndexOutOfRange("Unable to borrow a mutable reference to holon at supplied staged_index".to_string()))
            }
        },
        _ => Err(HolonError::InvalidParameter("Expected Command(StagedIndex) DanceType, didn't get one".to_string()))
    }
}


///
/// Builds a DanceRequest for adding a new property value(s) to an already staged holon.
pub fn build_with_properties_dance_request(staging_area: StagingArea, index: StagedIndex, properties: PropertyMap) ->Result<DanceRequest, HolonError> {
    let body = RequestBody::new_parameter_values(properties);
    Ok(DanceRequest::new(MapString("with_properties".to_string()), DanceType::Command(index), body, staging_area))
}


/// This dance retrieves all holons from the persistent store
pub fn get_all_holons_dance(_context: &HolonsContext, _request: DanceRequest) -> Result<Option<ResponseBody>, HolonError> {
    // TODO: add support for descriptor parameter
    //
    //
    let query_result =  Holon::get_all_holons();
    match query_result {
        Ok(holons) => Ok(
            Some(ResponseBody::Holons(holons))
        ),
        Err(holon_error) => {
            Err(holon_error.into())
        }
    }


}
// Builds a DanceRequest for retrieving all holons from the persistent store
pub fn build_get_all_holons_dance_request(staging_area: StagingArea)->Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(MapString("get_all_holons".to_string()), DanceType::Standalone, body, staging_area))
}

/// Commit all staged holons to the persistent store
///
pub fn commit_dance(context: &HolonsContext, _request: DanceRequest) -> Result<Option<ResponseBody>, HolonError> {

    let commit_response = context
        .commit_manager
        .borrow_mut()
        .commit(context);

    match commit_response.status {
        Success => Ok(None),
        Error(errors)
            => Err(HolonError::CommitFailure(HolonError::combine_errors(errors))),
    }
}

///
/// Builds a DanceRequest for staging a new holon. Properties, if supplied, they will be included
/// in the body of the request.
pub fn build_commit_dance_request(staging_area: StagingArea)->Result<DanceRequest, HolonError> {
    let body = RequestBody::None;
    Ok(DanceRequest::new(MapString("commit".to_string()), DanceType::Standalone, body, staging_area))
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
