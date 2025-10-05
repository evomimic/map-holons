use serde::{Deserialize, Serialize};
use std::fmt;

use crate::core_shared_objects::{summarize_holons, Holon, ReadableHolonState};
use crate::dances::SessionState;
use crate::query_layer::NodeCollection;
use crate::{HolonCollection, HolonReference};
use base_types::MapString;
use core_types::HolonError;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DanceResponse {
    pub status_code: ResponseStatusCode,
    pub description: MapString,
    pub body: ResponseBody,
    pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
    pub state: Option<SessionState>,
}

/// Define a standard set of statuses that may be returned by DanceRequests.
/// They are patterned after and should align, as much as reasonable, with [HTTP Status Codes](https://en.wikipedia.org/wiki/List_of_HTTP_status_codes)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResponseStatusCode {
    OK,                  // 200
    Accepted,            // 202
    BadRequest,          // 400
    Unauthorized,        // 401
    Forbidden,           // 403 -- use this for authorization / permission errors
    NotFound,            // 404
    Conflict, // 409 -- use this when request denied due to a conflict with the current state of the resource
    UnprocessableEntity, // 422 -- use this for semantic validation errors
    ServerError, // 500
    NotImplemented, // 501
    ServiceUnavailable, // 503
}

// Read-only results can be returned directly in ResponseBody as either a Holon or a
// (serialized) SmartCollection
// Staged holons will be returned via the StagingArea.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResponseBody {
    None,
    Holon(Holon),
    HolonCollection(HolonCollection),
    Holons(Vec<Holon>), // will be replaced by SmartCollection once supported
    HolonReference(HolonReference),
    NodeCollection(NodeCollection),
    // SmartCollection(SmartCollection),
}

impl From<HolonError> for ResponseStatusCode {
    fn from(error: HolonError) -> Self {
        match error {
            HolonError::CacheError(_) => ResponseStatusCode::ServerError,
            HolonError::CommitFailure(_) => ResponseStatusCode::ServerError,
            HolonError::DeletionNotAllowed(_) => ResponseStatusCode::Conflict,
            HolonError::DowncastFailure(_) => ResponseStatusCode::ServerError,
            HolonError::DuplicateError(_, _) => ResponseStatusCode::Conflict,
            HolonError::EmptyField(_) => ResponseStatusCode::BadRequest,
            HolonError::FailedToBorrow(_) => ResponseStatusCode::ServerError,
            HolonError::HashConversion(_, _) => ResponseStatusCode::ServerError,
            HolonError::HolonNotFound(_) => ResponseStatusCode::NotFound,
            HolonError::IndexOutOfRange(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidHolonReference(_) => ResponseStatusCode::BadRequest,
            HolonError::InvalidParameter(_) => ResponseStatusCode::BadRequest,
            HolonError::InvalidRelationship(_, _) => ResponseStatusCode::BadRequest,
            HolonError::InvalidTransition(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidType(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidUpdate(_) => ResponseStatusCode::ServerError,
            HolonError::Misc(_) => ResponseStatusCode::ServerError,
            HolonError::MissingStagedCollection(_) => ResponseStatusCode::BadRequest,
            HolonError::NotAccessible(_, _) => ResponseStatusCode::Conflict,
            HolonError::NotImplemented(_) => ResponseStatusCode::NotImplemented,
            HolonError::RecordConversion(_) => ResponseStatusCode::ServerError,
            HolonError::UnableToAddHolons(_) => ResponseStatusCode::ServerError,
            HolonError::UnexpectedValueType(_, _) => ResponseStatusCode::ServerError,
            HolonError::Utf8Conversion(_, _) => ResponseStatusCode::ServerError,
            HolonError::ValidationError(_) => ResponseStatusCode::UnprocessableEntity,
            HolonError::WasmError(_) => ResponseStatusCode::ServerError,
        }
    }
}
impl fmt::Display for ResponseStatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResponseStatusCode::OK => write!(f, "200 -- OK"),
            ResponseStatusCode::Accepted => write!(f, "202 -- Accepted"),
            ResponseStatusCode::BadRequest => write!(f, "400 -- Bad Request"),
            ResponseStatusCode::Unauthorized => write!(f, "401 -- Unauthorized"),
            ResponseStatusCode::Forbidden => write!(f, "403 -- Unauthorized"),
            ResponseStatusCode::NotFound => write!(f, "404 -- Not Found"),
            ResponseStatusCode::Conflict => write!(f, "409 -- Conflict"),
            ResponseStatusCode::ServerError => write!(f, "500 -- ServerError"),
            ResponseStatusCode::NotImplemented => write!(f, "501 -- Not Implemented"),
            ResponseStatusCode::ServiceUnavailable => write!(f, "503 -- Service Unavailable"),
            ResponseStatusCode::UnprocessableEntity => write!(f, "422 -- Unprocessable Entity"),
        }
    }
}

impl DanceResponse {
    pub fn new(
        status_code: ResponseStatusCode,
        description: MapString,
        body: ResponseBody,
        descriptor: Option<HolonReference>,
        state: Option<SessionState>,
    ) -> DanceResponse {
        DanceResponse { status_code, description, body, descriptor, state }
    }

    //moved to the dancer
    /*pub fn restore_state(&mut self, context: &dyn HolonsContextBehavior) {
        let space_manager = &context.get_space_manager();
        let staged_holons = space_manager.get_holon_stage();
        let staged_index = space_manager.get_stage_key_index();
        let staging_area = StagingArea::new_from_references(staged_holons, staged_index);
        let local_space_holon = space_manager.get_space_holon();
        self.state.set_staging_area(staging_area);
        self.state.set_local_holon_space(local_space_holon);
    }*/
    // Method to summarize the DanceResponse for logging purposes
    pub fn summarize(&self) -> String {
        let body_summary = match &self.body {
            ResponseBody::Holon(holon) => holon.summarize(),
            ResponseBody::Holons(holons) => summarize_holons(holons),
            _ => format!("{:#?}", self.body), // Use full debug for other response bodies
        };

        format!(
            "DanceResponse {{ \n  status_code: {:?}, \n  description: {:?}, \n  descriptor: {:?}, \n  body: {},\n  state: {} }}",
            self.status_code,
            self.description,
            self.descriptor,
            body_summary,
            self.state
                .as_ref()
                .map_or_else(|| "None".to_string(), |state| state.summarize()),
        )
    }
}
