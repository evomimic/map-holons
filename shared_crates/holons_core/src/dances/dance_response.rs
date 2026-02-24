use crate::core_shared_objects::{holon::summarize_holons, Holon, ReadableHolonState};
use crate::query_layer::NodeCollection;
use crate::{HolonCollection, HolonReference};
use base_types::MapString;
use core_types::HolonError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Runtime dance response (tx-bound, execution-capable).
///
/// This type must not be deserialized across IPC boundaries because it may contain
/// tx-bound references. Use `DanceResponseWire` for IPC and call `bind(context)` at ingress.
#[derive(Debug, Clone, PartialEq)]
pub struct DanceResponse {
    pub status_code: ResponseStatusCode,
    pub description: MapString,
    pub body: ResponseBody,
    pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
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
/// Runtime response body (may contain tx-bound references).
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseBody {
    None,
    Holon(Holon),
    HolonCollection(HolonCollection),
    Holons(Vec<Holon>), // will be replaced by SmartCollection once supported
    HolonReference(HolonReference),
    NodeCollection(NodeCollection),
    // SmartCollection(SmartCollection),
}

impl DanceResponse {
    pub fn new(
        status_code: ResponseStatusCode,
        description: MapString,
        body: ResponseBody,
        descriptor: Option<HolonReference>,
    ) -> DanceResponse {
        DanceResponse { status_code, description, body, descriptor }
    }

    /// Annotates this response with a local processing error (e.g. envelope hydration failure).
    ///
    /// Preserves existing fields (body, descriptor, etc.) but:
    /// - Updates `status_code` to reflect the mapped error code.
    /// - Appends a diagnostic note to the `description`, including both
    ///   the local error and the original response status.
    pub fn annotate_error(&mut self, error: HolonError) {
        let prev_code = self.status_code.clone();
        let new_code = ResponseStatusCode::from(error.clone());

        let note = format!(
            "[Local processing error: {} → mapped to {} (original response was {})]",
            error, new_code, prev_code
        );

        // Update the status code
        self.status_code = new_code;

        // Append to or initialize description
        if self.description.0.is_empty() {
            self.description = MapString(note);
        } else {
            self.description = MapString(format!("{}\n{}", self.description.0, note));
        }
    }
    /// Constructs a `DanceResponse` representing an error that occurred during a dance.
    ///
    /// This method wraps the provided [`HolonError`] into a [`DanceResponse`] object
    /// by:
    /// - Mapping the error to a [`ResponseStatusCode`] using its `From<HolonError>` implementation.
    /// - Converting the error message into a `MapString` description.
    /// - Setting `body` to `ResponseBody::None` (no successful payload).
    /// - Leaving `descriptor` unset (`None`).
    pub fn from_error(error: HolonError) -> Self {
        Self {
            status_code: ResponseStatusCode::from(error.clone()),
            description: MapString(error.to_string()),
            body: ResponseBody::None,
            descriptor: None,
        }
    }

    // Method to summarize the DanceResponse for logging purposes.
    // Keep this intentionally shallow to avoid traversing tx-bound runtime graphs in logs.
    pub fn summarize(&self) -> String {
        let body_summary = self.body_summary();
        let descriptor_summary = self.descriptor_summary();

        format!(
            "DanceResponse {{ status_code: {:?}, description: {:?}, descriptor: {}, body: {} }}",
            self.status_code, self.description, descriptor_summary, body_summary,
        )
    }

    fn descriptor_summary(&self) -> String {
        match &self.descriptor {
            None => "none".to_string(),
            Some(HolonReference::Smart(_)) => "smart".to_string(),
            Some(HolonReference::Staged(_)) => "staged".to_string(),
            Some(HolonReference::Transient(_)) => "transient".to_string(),
        }
    }

    fn body_summary(&self) -> String {
        match &self.body {
            ResponseBody::None => "None".to_string(),
            ResponseBody::Holon(holon) => {
                // Holon summaries are bounded and useful for test logging.
                format!("Holon({})", holon.summarize())
            }
            ResponseBody::Holons(holons) => {
                format!("Holons(count={}, {})", holons.len(), summarize_holons(holons))
            }
            ResponseBody::HolonCollection(collection) => {
                let count = collection.get_members().len();
                format!("HolonCollection(state={:?}, count={})", collection.get_state(), count)
            }
            ResponseBody::HolonReference(reference) => match reference {
                HolonReference::Smart(smart) => {
                    format!("HolonReference::Smart(holon_id={})", smart.holon_id())
                }
                HolonReference::Staged(staged) => {
                    format!("HolonReference::Staged(id={})", staged.temporary_id())
                }
                HolonReference::Transient(transient) => {
                    format!("HolonReference::Transient(id={})", transient.temporary_id())
                }
            },
            ResponseBody::NodeCollection(nodes) => {
                format!("NodeCollection(count={})", nodes.members.len())
            }
        }
    }
}

impl From<HolonError> for ResponseStatusCode {
    fn from(error: HolonError) -> Self {
        match error {
            // 500-ish (internal / infrastructure)
            HolonError::CacheError(_) => ResponseStatusCode::ServerError,
            HolonError::CommitFailure(_) => ResponseStatusCode::ServerError,
            HolonError::ConductorError(_) => ResponseStatusCode::ServerError,
            HolonError::DowncastFailure(_) => ResponseStatusCode::ServerError,
            HolonError::FailedToBorrow(_) => ResponseStatusCode::ServerError,
            HolonError::FailedToAcquireLock(_) => ResponseStatusCode::ServerError,
            HolonError::HashConversion(_, _) => ResponseStatusCode::ServerError,
            HolonError::IndexOutOfRange(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidType(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidUpdate(_) => ResponseStatusCode::ServerError,
            HolonError::RecordConversion(_) => ResponseStatusCode::ServerError,
            HolonError::ServiceNotAvailable(_) => ResponseStatusCode::ServiceUnavailable,
            HolonError::UnableToAddHolons(_) => ResponseStatusCode::ServerError,
            HolonError::UnexpectedValueType(_, _) => ResponseStatusCode::ServerError,
            HolonError::Utf8Conversion(_, _) => ResponseStatusCode::ServerError,
            HolonError::WasmError(_) => ResponseStatusCode::ServerError,
            HolonError::Misc(_) => ResponseStatusCode::ServerError,

            // 404-ish (missing resource)
            HolonError::HolonNotFound(_) => ResponseStatusCode::NotFound,

            // 409-ish (conflict with current state / invariants)
            HolonError::CrossTransactionReference { .. } => ResponseStatusCode::Conflict,
            HolonError::DeletionNotAllowed(_) => ResponseStatusCode::Conflict,
            HolonError::DuplicateError(_, _) => ResponseStatusCode::Conflict,
            HolonError::InvalidTransition(_) => ResponseStatusCode::ServerError,
            HolonError::InvalidTransactionTransition { .. } => ResponseStatusCode::Conflict,
            HolonError::NotAccessible(_, _) => ResponseStatusCode::Conflict,
            HolonError::TransactionAlreadyCommitted { .. } => ResponseStatusCode::Conflict,
            HolonError::TransactionCommitInProgress { .. } => ResponseStatusCode::Conflict,
            HolonError::TransactionNotOpen { .. } => ResponseStatusCode::Conflict,

            // 400-ish (client supplied invalid input / malformed request)
            HolonError::EmptyField(_) => ResponseStatusCode::BadRequest,
            HolonError::InvalidHolonReference(_) => ResponseStatusCode::BadRequest,
            HolonError::InvalidParameter(_) => ResponseStatusCode::BadRequest,
            HolonError::InvalidRelationship(_, _) => ResponseStatusCode::BadRequest,
            HolonError::InvalidWireFormat { .. } => ResponseStatusCode::BadRequest,
            HolonError::MissingStagedCollection(_) => ResponseStatusCode::BadRequest,

            // 422-ish (semantic validation / parse errors)
            HolonError::LoaderParsingError(_) => ResponseStatusCode::UnprocessableEntity,
            HolonError::ReferenceBindingFailed { .. } => ResponseStatusCode::UnprocessableEntity,
            HolonError::ReferenceResolutionFailed { .. } => ResponseStatusCode::UnprocessableEntity,
            HolonError::ValidationError(_) => ResponseStatusCode::UnprocessableEntity,

            // 501-ish
            HolonError::NotImplemented(_) => ResponseStatusCode::NotImplemented,
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
            ResponseStatusCode::Forbidden => write!(f, "403 -- Forbidden"),
            ResponseStatusCode::NotFound => write!(f, "404 -- Not Found"),
            ResponseStatusCode::Conflict => write!(f, "409 -- Conflict"),
            ResponseStatusCode::ServerError => write!(f, "500 -- ServerError"),
            ResponseStatusCode::NotImplemented => write!(f, "501 -- Not Implemented"),
            ResponseStatusCode::ServiceUnavailable => write!(f, "503 -- Service Unavailable"),
            ResponseStatusCode::UnprocessableEntity => write!(f, "422 -- Unprocessable Entity"),
        }
    }
}
