use base_types::BaseValue;
use holons_boundary::{
    DanceResponseWire, HolonCollectionWire, HolonReferenceWire, HolonWire, NodeCollectionWire,
    TransientReferenceWire,
};
use holons_core::core_shared_objects::transactions::TxId;
use serde::{Deserialize, Serialize};

use crate::domain::MapResult;

/// Serializable result variants for MAP Command responses.
///
/// These represent the successful return values from command execution,
/// serialized for IPC transport back to the TypeScript client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MapResultWire {
    /// Command completed with no return value.
    Unit,

    /// Returns a new transaction id (from BeginTransaction).
    TransactionCreated { tx_id: TxId },

    /// Returns a committed transaction result.
    Committed,

    /// Returns a transient reference (from CreateTransientHolon).
    TransientReference(TransientReferenceWire),

    /// Returns a holon reference (from staging, versioning, etc.).
    HolonReference(HolonReferenceWire),

    /// Returns a single holon.
    Holon(HolonWire),

    /// Returns a collection of holons.
    HolonCollection(HolonCollectionWire),

    /// Returns a node collection (query result).
    NodeCollection(NodeCollectionWire),

    /// Returns a single property value.
    PropertyValue(Option<BaseValue>),

    /// Returns a dance response (for Dance action passthrough).
    DanceResponse(DanceResponseWire),
}

/// Wire-level response body for dance results.
///
/// Subset of ResponseBodyWire, scoped to what MAP Commands can return.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResponseBodyResultWire {
    None,
    Holon(HolonWire),
    HolonCollection(HolonCollectionWire),
    Holons(Vec<HolonWire>),
    HolonReference(HolonReferenceWire),
    NodeCollection(NodeCollectionWire),
}

impl From<MapResult> for MapResultWire {
    fn from(result: MapResult) -> Self {
        match result {
            MapResult::Unit => MapResultWire::Unit,
            MapResult::TransactionCreated { tx_id } => MapResultWire::TransactionCreated { tx_id },
            MapResult::Committed => MapResultWire::Committed,
            MapResult::TransientReference(r) => {
                MapResultWire::TransientReference(TransientReferenceWire::from(&r))
            }
            MapResult::HolonReference(r) => {
                MapResultWire::HolonReference(HolonReferenceWire::from(&r))
            }
            MapResult::Holon(h) => MapResultWire::Holon(HolonWire::from(&h)),
            MapResult::HolonCollection(c) => {
                MapResultWire::HolonCollection(HolonCollectionWire::from(&c))
            }
            MapResult::NodeCollection(n) => {
                MapResultWire::NodeCollection(NodeCollectionWire::from(&n))
            }
            MapResult::PropertyValue(v) => MapResultWire::PropertyValue(v),
            MapResult::DanceResponse(r) => {
                MapResultWire::DanceResponse(DanceResponseWire::from(&r))
            }
        }
    }
}
