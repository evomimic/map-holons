use base_types::{BaseValue, MapString};
use core_types::HolonId;
use holons_boundary::{
    DanceResponseWire, HolonCollectionWire, HolonReferenceWire, NodeCollectionWire,
};
use holons_core::core_shared_objects::holon::EssentialHolonContent;
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
    None,

    /// Returns a new transaction id (from BeginTransaction).
    TransactionCreated { tx_id: TxId },

    /// Returns a committed transaction result.
    CommitResponse(HolonReferenceWire),

    /// Returns a holon reference.
    HolonReference(HolonReferenceWire),

    /// Returns a collection of holon references.
    HolonReferences(Vec<HolonReferenceWire>),

    /// Returns an indexed collection of holons.
    HolonCollection(HolonCollectionWire),

    /// Returns a node collection (query result).
    NodeCollection(NodeCollectionWire),

    /// Returns a single property value.
    PropertyValue(Option<BaseValue>),

    /// Returns a string value (e.g. from `versioned_key()`).
    StringValue(MapString),

    /// Returns a holon id.
    HolonId(HolonId),

    /// Returns the essential content of a holon.
    EssentialContent(EssentialHolonContent),

    /// Returns a dance response.
    DanceResponse(DanceResponseWire),
}

impl From<MapResult> for MapResultWire {
    fn from(result: MapResult) -> Self {
        match result {
            MapResult::None => MapResultWire::None,
            MapResult::TransactionCreated { tx_id } => {
                MapResultWire::TransactionCreated { tx_id }
            }
            MapResult::CommitResponse(r) => {
                MapResultWire::CommitResponse(HolonReferenceWire::from(&r))
            }
            MapResult::HolonReference(r) => {
                MapResultWire::HolonReference(HolonReferenceWire::from(&r))
            }
            MapResult::HolonReferences(refs) => {
                MapResultWire::HolonReferences(
                    refs.iter().map(HolonReferenceWire::from).collect(),
                )
            }
            MapResult::HolonCollection(c) => {
                MapResultWire::HolonCollection(HolonCollectionWire::from(&c))
            }
            MapResult::NodeCollection(n) => {
                MapResultWire::NodeCollection(NodeCollectionWire::from(&n))
            }
            MapResult::PropertyValue(v) => MapResultWire::PropertyValue(v),
            MapResult::StringValue(s) => MapResultWire::StringValue(s),
            MapResult::HolonId(id) => MapResultWire::HolonId(id),
            MapResult::EssentialContent(c) => MapResultWire::EssentialContent(c),
            MapResult::DanceResponse(r) => {
                MapResultWire::DanceResponse(DanceResponseWire::from(&r))
            }
        }
    }
}
