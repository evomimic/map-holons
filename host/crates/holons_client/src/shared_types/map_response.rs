use holons_boundary::{HolonReferenceWire, ResponseBodyWire};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    dances::{DanceResponse, ResponseBody, ResponseStatusCode},
    HolonError, HolonReference,
};
use holons_boundary::session_state::SessionStateWire;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MapResponse {
    pub space_id: String,
    pub status_code: ResponseStatusCode,
    pub description: String,
    pub body: ResponseBody,
    pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
    pub state: Option<SessionStateWire>,
}

/// IPC-safe wire-form map response.
///
/// This is the context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime via `bind(context)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapResponseWire {
    pub space_id: String,
    pub status_code: ResponseStatusCode,
    pub description: String,
    pub body: ResponseBodyWire,
    pub descriptor: Option<HolonReferenceWire>, // space_id+holon_id of DanceDescriptor
    pub state: Option<SessionStateWire>,
}

impl MapResponse {
    pub fn new_from_dance_response(space_id: String, dance_response: DanceResponse) -> Self {
        Self {
            space_id,
            status_code: dance_response.status_code,
            description: dance_response.description.to_string(),
            body: dance_response.body,
            descriptor: dance_response.descriptor,
            state: None,
        }
    }
}

impl MapResponseWire {
    /// Binds a wire response to the supplied transaction, validating `tx_id`
    /// for all embedded references.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<MapResponse, HolonError> {
        Ok(MapResponse {
            space_id: self.space_id,
            status_code: self.status_code,
            description: self.description,
            body: self.body.bind(context)?,
            descriptor: match self.descriptor {
                Some(reference_wire) => Some(reference_wire.bind(context)?),
                None => None,
            },
            state: self.state,
        })
    }
}

impl From<&MapResponse> for MapResponseWire {
    fn from(response: &MapResponse) -> Self {
        Self {
            space_id: response.space_id.clone(),
            status_code: response.status_code.clone(),
            description: response.description.clone(),
            body: ResponseBodyWire::from(&response.body),
            descriptor: response.descriptor.as_ref().map(HolonReferenceWire::from),
            state: response.state.clone(),
        }
    }
}
