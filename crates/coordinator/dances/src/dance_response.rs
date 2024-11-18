use derive_new::new;
use std::fmt;

use crate::session_state::SessionState;
use crate::staging_area::StagingArea;
use hdk::prelude::*;
use holons::commit_manager::StagedIndex;
use holons::context::HolonsContext;
use holons::helpers::summarize_holons;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::query::NodeCollection;
use shared_types_holon::MapString;

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct DanceResponse {
    pub status_code: ResponseStatusCode,
    pub description: MapString,
    pub body: ResponseBody,
    pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
    pub state: SessionState,
}

/// Define a standard set of statuses that may be returned by DanceRequests.
/// They are patterned after and should align, as much as reasonable, with [HTTP Status Codes](https://en.wikipedia.org/wiki/List_of_HTTP_status_codes)
#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub enum ResponseStatusCode {
    OK,                 // 200
    Accepted,           // 202
    BadRequest,         // 400
    Unauthorized,       // 401
    Forbidden,          // 403 -- use this for authorization / permission errors
    NotFound,           // 404
    Conflict, // 409 -- use this when request denied due to a conflict with the current state of the resource
    ServerError, // 500
    NotImplemented, // 501
    ServiceUnavailable, // 503
}

// Read-only results can be returned directly in ResponseBody as either a Holon or a
// (serialized) SmartCollection
// Staged holons will be returned via the StagingArea.
// StagedIndex is used to return a (reference) to a StagedHolon
#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum ResponseBody {
    None,
    Holon(Holon),
    Holons(Vec<Holon>), // will be replaced by SmartCollection once supported
    // SmartCollection(SmartCollection),
    Index(StagedIndex),
    HolonReference(HolonReference),
    Collection(NodeCollection),
}

impl From<HolonError> for ResponseStatusCode {
    fn from(error: HolonError) -> Self {
        match error {
            HolonError::CacheError(_) => ResponseStatusCode::ServerError,
            HolonError::CommitFailure(_) => ResponseStatusCode::ServerError,
            HolonError::DeletionNotAllowed(_) => ResponseStatusCode::Conflict,
            HolonError::EmptyField(_) => ResponseStatusCode::BadRequest,
            HolonError::FailedToBorrow(_) => ResponseStatusCode::ServerError,
            HolonError::HashConversion(_, _) => ResponseStatusCode::ServerError,
            HolonError::HolonNotFound(_) => ResponseStatusCode::NotFound,
            HolonError::IndexOutOfRange(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidHolonReference(_) => ResponseStatusCode::BadRequest,
            HolonError::InvalidParameter(_) => ResponseStatusCode::BadRequest,
            HolonError::InvalidRelationship(_, _) => ResponseStatusCode::BadRequest,
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
            HolonError::ValidationError(_) => ResponseStatusCode::BadRequest,
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
        }
    }
}

impl DanceResponse {
    pub fn new(
        status_code: ResponseStatusCode,
        description: MapString,
        body: ResponseBody,
        descriptor: Option<HolonReference>,
        state: SessionState,
    ) -> DanceResponse {
        DanceResponse { status_code, description, body, descriptor, state }
    }
    /// Restores the session state within the DanceResponse from context. This should always
    /// be called before returning DanceResponse since the state is intended to be "ping-ponged"
    /// between client and guest.
    /// NOTE: Errors in restoring the state are not handled (i.e., will cause panic)
    pub fn restore_state(&mut self, context: &HolonsContext) {
        self.state
            .set_staging_area(StagingArea::from_commit_manager(&context.commit_manager.borrow()));
        self.state.set_local_holon_space(context.get_local_space_holon());
    }
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
            self.state.summarize(),
        )
    }
}
