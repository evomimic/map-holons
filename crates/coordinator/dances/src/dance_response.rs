use derive_new::new;
use std::fmt;

use crate::holon_dance_adapter::NodeCollection;
use crate::staging_area::StagingArea;
use hdk::prelude::*;
use holons::commit_manager::StagedIndex;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use shared_types_holon::MapString;

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct DanceResponse {
    pub status_code: ResponseStatusCode,
    pub description: MapString,
    pub body: ResponseBody,
    pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
    pub staging_area: StagingArea,
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
            HolonError::EmptyField(_) => ResponseStatusCode::BadRequest,
            HolonError::InvalidParameter(_) => ResponseStatusCode::BadRequest,
            HolonError::HolonNotFound(_) => ResponseStatusCode::NotFound,
            HolonError::CommitFailure(_) => ResponseStatusCode::ServerError,
            HolonError::WasmError(_) => ResponseStatusCode::ServerError,
            HolonError::RecordConversion(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidHolonReference(_) => ResponseStatusCode::BadRequest,
            HolonError::IndexOutOfRange(_) => ResponseStatusCode::ServerError,
            HolonError::NotImplemented(_) => ResponseStatusCode::NotImplemented,
            HolonError::MissingStagedCollection(_) => ResponseStatusCode::BadRequest,
            HolonError::FailedToBorrow(_) => ResponseStatusCode::ServerError,
            HolonError::UnableToAddHolons(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidRelationship(_, _) => ResponseStatusCode::ServerError,
            HolonError::CacheError(_) => ResponseStatusCode::ServerError,
            HolonError::NotAccessible(_, _) => ResponseStatusCode::Conflict,
            HolonError::ValidationError(_) => ResponseStatusCode::BadRequest,
            HolonError::Utf8Conversion(_, _) => ResponseStatusCode::ServerError,
            HolonError::HashConversion(_, _) => ResponseStatusCode::ServerError,
            HolonError::UnexpectedValueType(_, _) => ResponseStatusCode::ServerError,
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
        staging_area: StagingArea,
    ) -> DanceResponse {
        DanceResponse {
            status_code,
            description,
            body,
            descriptor,
            staging_area,
        }
    }
}
