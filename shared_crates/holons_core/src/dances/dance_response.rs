use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{
    summarize_holons, Holon, HolonCollectionWire, HolonWire, ReadableHolonState,
};
use crate::dances::SessionState;
use crate::query_layer::{NodeCollection, NodeCollectionWire};
use crate::{HolonCollection, HolonReference, HolonReferenceWire};
use base_types::MapString;
use core_types::HolonError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

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
    pub state: Option<SessionState>,
}

/// IPC-safe wire-form dance response.
///
/// This is the context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime via `bind(context)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DanceResponseWire {
    pub status_code: ResponseStatusCode,
    pub description: MapString,
    pub body: ResponseBodyWire,
    pub descriptor: Option<HolonReferenceWire>,
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

/// IPC-safe wire-form response body.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResponseBodyWire {
    None,
    Holon(HolonWire),
    HolonCollection(HolonCollectionWire),
    Holons(Vec<HolonWire>),
    HolonReference(HolonReferenceWire),
    NodeCollection(NodeCollectionWire),
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
    /// - Leaving `descriptor` and `state` unset (`None`).
    pub fn from_error(error: HolonError) -> Self {
        Self {
            status_code: ResponseStatusCode::from(error.clone()),
            description: MapString(error.to_string()),
            body: ResponseBody::None,
            descriptor: None,
            state: None,
        }
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
            self.state
                .as_ref()
                .map_or_else(|| "None".to_string(), |state| state.summarize()),
        )
    }
}

impl DanceResponseWire {
    /// Binds a wire response to the supplied transaction, validating `tx_id` for all embedded references.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<DanceResponse, HolonError> {
        Ok(DanceResponse {
            status_code: self.status_code,
            description: self.description,
            body: self.body.bind(context)?,
            descriptor: match self.descriptor {
                None => None,
                Some(reference_wire) => Some(HolonReference::bind(reference_wire, context)?),
            },
            state: self.state,
        })
    }

    /// Summarizes the IPC-safe wire response for logging purposes.
    ///
    /// Safe to call immediately after IPC decode (before binding).
    pub fn summarize(&self) -> String {
        let state_summary =
            self.state.as_ref().map_or_else(|| "None".to_string(), |state| state.summarize());

        format!(
            "DanceResponseWire {{ \n  status_code: {}, \n  description: {:?}, \n  descriptor: {:?}, \n  body: {},\n  state: {} }}",
            self.status_code,
            self.description,
            self.descriptor,
            self.body.summarize(),
            state_summary,
        )
    }
}

impl ResponseBodyWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<ResponseBody, HolonError> {
        match self {
            ResponseBodyWire::None => Ok(ResponseBody::None),
            ResponseBodyWire::Holon(holon_wire) => {
                Ok(ResponseBody::Holon(holon_wire.bind(Arc::clone(context))?))
            }
            ResponseBodyWire::HolonCollection(collection_wire) => {
                Ok(ResponseBody::HolonCollection(collection_wire.bind(Arc::clone(context))?))
            }
            ResponseBodyWire::Holons(holons_wire) => {
                let mut holons = Vec::with_capacity(holons_wire.len());
                for holon_wire in holons_wire {
                    holons.push(holon_wire.bind(Arc::clone(context))?);
                }
                Ok(ResponseBody::Holons(holons))
            }
            ResponseBodyWire::HolonReference(reference_wire) => {
                Ok(ResponseBody::HolonReference(HolonReference::bind(reference_wire, context)?))
            }
            ResponseBodyWire::NodeCollection(node_collection_wire) => {
                Ok(ResponseBody::NodeCollection(node_collection_wire.bind(context)?))
            }
        }
    }

    /// Summarizes the wire response body for logging purposes.
    pub fn summarize(&self) -> String {
        match self {
            ResponseBodyWire::None => "None".to_string(),

            ResponseBodyWire::Holon(holon_wire) => {
                format!("HolonWire: {:#?}", holon_wire)
            }

            ResponseBodyWire::HolonCollection(collection_wire) => {
                format!("HolonCollectionWire ({} holons)", collection_wire.summarize())
            }

            ResponseBodyWire::Holons(holons_wire) => {
                format!("HolonsWire ({} holons)", holons_wire.len())
            }

            ResponseBodyWire::HolonReference(reference_wire) => {
                format!("HolonReferenceWire: {:?}", reference_wire)
            }

            ResponseBodyWire::NodeCollection(node_collection_wire) => {
                format!("NodeCollectionWire: {:#?}", node_collection_wire)
            }
        }
    }
}

impl From<&DanceResponse> for DanceResponseWire {
    fn from(response: &DanceResponse) -> Self {
        Self {
            status_code: response.status_code.clone(),
            description: response.description.clone(),
            body: ResponseBodyWire::from(&response.body),
            descriptor: response.descriptor.as_ref().map(HolonReferenceWire::from),
            state: response.state.clone(),
        }
    }
}

impl From<&ResponseBody> for ResponseBodyWire {
    fn from(body: &ResponseBody) -> Self {
        match body {
            ResponseBody::None => ResponseBodyWire::None,
            ResponseBody::Holon(holon) => ResponseBodyWire::Holon(HolonWire::from(holon)),
            ResponseBody::HolonCollection(collection) => {
                ResponseBodyWire::HolonCollection(HolonCollectionWire::from(collection))
            }
            ResponseBody::Holons(holons) => {
                ResponseBodyWire::Holons(holons.iter().map(HolonWire::from).collect())
            }
            ResponseBody::HolonReference(reference) => {
                ResponseBodyWire::HolonReference(HolonReferenceWire::from(reference))
            }
            ResponseBody::NodeCollection(node_collection) => {
                ResponseBodyWire::NodeCollection(NodeCollectionWire::from(node_collection))
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
            HolonError::InvalidTransition(_) => ResponseStatusCode::ServerError,
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
            HolonError::NotAccessible(_, _) => ResponseStatusCode::Conflict,

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
