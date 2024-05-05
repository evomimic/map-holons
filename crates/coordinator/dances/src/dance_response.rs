use derive_new::new;

use crate::staging_area::StagingArea;
use hdk::prelude::*;
use holons::holon::Holon;
use holons::holon_errors::HolonError;
use holons::holon_reference::HolonReference;
use holons::smart_collection::SmartCollection;
use shared_types_holon::{MapInteger, MapString};

/// Define a standard set of statuses that may be returned by DanceRequests.
/// They are patterned after and should align, as much as reasonable, with [HTTP Status Codes](https://en.wikipedia.org/wiki/List_of_HTTP_status_codes)
#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub enum ResponseStatusCode {
    Ok,                 // 200
    Accepted,           // 202
    BadRequest,         // 400,
    Unauthorized,       // 401
    NotFound,           // 404
    ServerError,        // 500
    NotImplemented,     // 501
    ServiceUnavailable, // 503
}

impl From<HolonError> for ResponseStatusCode {
    fn from(error: HolonError) -> Self {
        match error {
            HolonError::EmptyField(_) => ResponseStatusCode::BadRequest,
            HolonError::HolonNotFound(_) => ResponseStatusCode::NotFound,
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
            HolonError::ValidationError(_) => ResponseStatusCode::BadRequest,
        }
    }
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct DanceResponse {
    pub status_code: ResponseStatusCode,
    pub description: MapString,
    pub body: Option<ResponseBody>,
    pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
    pub staging_area: Option<StagingArea>,
}
// Read-only results can be returned directly in ResponseBody as either a Holon or a
// (serialized) SmartCollection
// Staged holons will be returned via the StagingArea.
// StagedIndex is used to return a (reference) to a StagedHolon

pub type StagedIndex = MapInteger;
#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum ResponseBody {
    Holon(Holon),
    Holons(Vec<Holon>),
    SmartCollection(SmartCollection),
    Index(StagedIndex),
}

impl DanceResponse {
    pub fn new(
        status_code: ResponseStatusCode,
        description: MapString,
        body: Option<ResponseBody>,
        descriptor: Option<HolonReference>,
        staging_area: Option<StagingArea>,
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
