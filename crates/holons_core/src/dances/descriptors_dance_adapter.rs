#![allow(unused_variables)]
//! This file defines the DancesAdaptors offered by the descriptors zome.
//! TODO: Move these adaptors to their own zome (at some point)
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

use crate::dances::{DanceRequest, DanceType, RequestBody, ResponseBody};
use crate::HolonsContextBehavior;
use base_types::MapString;
use core_types::HolonError;

/// *DanceRequest:*
/// - dance_name: "load_core_schema"
/// - dance_type: Standalone
/// - request_body: None
///
/// *ResponseBody:*
/// - Holon -- the created Schema Holon
///
pub fn load_core_schema_dance(
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError> {
    // TODO: Need to sort out the dependencies (find new home for descriptors_dance_adapter)
    todo!()
    // debug!("Entered load_core_schema_dance");
    //
    // // Match the dance_type
    // match request.dance_type {
    //     DanceType::Standalone => {
    //         // Call the native load_core_schema function
    //         let result = load_core_schema(context);
    //         match result {
    //             Ok(commit_response) => match commit_response.status {
    //                 CommitRequestStatus::Complete => Ok(ResponseBody::None),
    //                 CommitRequestStatus::Incomplete => {
    //                     Err(HolonError::CommitFailure("Incomplete commit".to_string()))
    //                 }
    //             },
    //             Err(e) => Err(e),
    //         }
    //     }
    //     _ => Err(HolonError::InvalidParameter(
    //         "Expected Standalone DanceType, didn't get one".to_string(),
    //     )),
    // }
}

pub fn build_load_core_schema_dance_request() -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(
        MapString("load_core_schema".to_string()),
        DanceType::Standalone,
        body,
        None,
    ))
}
