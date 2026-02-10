use crate::context_binding::holon_collection_wire::HolonCollectionWire;
use crate::context_binding::query_wire::NodeCollectionWire;
use crate::context_binding::HolonWire;
use crate::HolonReferenceWire;
use base_types::MapString;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceResponse, ResponseBody, ResponseStatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

impl DanceResponseWire {
    /// Binds a wire response to the supplied transaction, validating `tx_id` for all embedded references.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<DanceResponse, HolonError> {
        Ok(DanceResponse {
            status_code: self.status_code,
            description: self.description,
            body: self.body.bind(context)?,
            descriptor: match self.descriptor {
                None => None,
                Some(reference_wire) => Some(reference_wire.bind(context)?),
            },
        })
    }

    /// Summarizes the IPC-safe wire response for logging purposes.
    ///
    /// Safe to call immediately after IPC decode (before context_binding).
    pub fn summarize(&self) -> String {
        format!(
            "DanceResponseWire {{ \n  status_code: {}, \n  description: {:?}, \n  descriptor: {:?}, \n  body: {} }}",
            self.status_code,
            self.description,
            self.descriptor,
            self.body.summarize(),
        )
    }
}

impl ResponseBodyWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<ResponseBody, HolonError> {
        match self {
            ResponseBodyWire::None => Ok(ResponseBody::None),
            ResponseBodyWire::Holon(holon_wire) => {
                Ok(ResponseBody::Holon(holon_wire.bind(context)?))
            }
            ResponseBodyWire::HolonCollection(collection_wire) => {
                Ok(ResponseBody::HolonCollection(collection_wire.bind(context)?))
            }
            ResponseBodyWire::Holons(holons_wire) => {
                let mut holons = Vec::with_capacity(holons_wire.len());
                for holon_wire in holons_wire {
                    holons.push(holon_wire.bind(context)?);
                }
                Ok(ResponseBody::Holons(holons))
            }
            ResponseBodyWire::HolonReference(reference_wire) => {
                Ok(ResponseBody::HolonReference(reference_wire.bind(context)?))
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
