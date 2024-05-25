//! This file defines the DancesAdaptors offered by the descriptors zome.
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


use std::borrow::Borrow;
use std::rc::Rc;

use hdk::prelude::*;
use holons::commit_manager::CommitRequestStatus::*;
use holons::commit_manager::{CommitManager, StagedIndex};
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use descriptors::loader::load_core_schema;
use holons::relationship::RelationshipName;
use shared_types_holon::{MapString, MapInteger, PropertyMap};
use shared_types_holon::HolonId;



use crate::dance_request::{DanceRequest, DanceType, PortableReference, RequestBody};
use crate::dance_response::ResponseBody;
use crate::staging_area::StagingArea;

/// *DanceRequest:*
/// - dance_name: "load_core_schema"
/// - dance_type: Standalone
/// - request_body: None
///
/// *ResponseBody:*
/// - Holon -- the created Schema Holon
///
pub fn load_core_schema_dance(context: &HolonsContext, request: DanceRequest) -> Result<ResponseBody, HolonError> {
    debug!("Entered load_core_schema_dance");

    // Match the dance_type
    match request.dance_type {
        DanceType::Standalone=> {
            // Call the native load_core_schema function
            let result = load_core_schema(context);
            match result {
                Ok(schema)=> {
                    Ok(ResponseBody::Holon(schema))
                }
                Err(e) => Err(e),
            }
        }
        _ => Err(HolonError::InvalidParameter("Expected Standalone DanceType, didn't get one".to_string())),
    }
}


pub fn build_load_core_schema_dance_request(
    staging_area: StagingArea,
)->Result<DanceRequest, HolonError> {
    let body = RequestBody::new();
    Ok(DanceRequest::new(MapString("load_core_schema".to_string()), DanceType::Standalone,body, staging_area))
}

