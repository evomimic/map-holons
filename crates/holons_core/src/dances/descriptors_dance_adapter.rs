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

use core_schema::loader::load_core_schema;
use dances_core::dance_request::{DanceRequest, DanceType, RequestBody};
use dances_core::dance_response::ResponseBody;
use dances_core::session_state::SessionState;
use hdk::prelude::*;
use holons_core::core_shared_objects::{CommitRequestStatus, HolonError};
use holons_core::reference_layer::HolonsContextBehavior;
use shared_types_holon::MapString;

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

pub fn build_load_core_schema_dance_request(
    session_state: &SessionState,
) -> Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(
        MapString("load_core_schema".to_string()),
        DanceType::Standalone,
        body,
        session_state.clone(),
    ))
}
