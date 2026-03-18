use base_types::BaseValue;
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

    /// Returns a holon reference.
    Reference(HolonReferenceWire),

    /// Returns a collection of holon references.
    References(Vec<HolonReferenceWire>),

    /// Returns an indexed collection of holons.
    Collection(HolonCollectionWire),

    /// Returns a node collection (query result).
    NodeCollection(NodeCollectionWire),

    /// Universal scalar return.
    Value(BaseValue),

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
            MapResult::Reference(r) => {
                MapResultWire::Reference(HolonReferenceWire::from(&r))
            }
            MapResult::References(refs) => {
                MapResultWire::References(
                    refs.iter().map(HolonReferenceWire::from).collect(),
                )
            }
            MapResult::Collection(c) => {
                MapResultWire::Collection(HolonCollectionWire::from(&c))
            }
            MapResult::NodeCollection(n) => {
                MapResultWire::NodeCollection(NodeCollectionWire::from(&n))
            }
            MapResult::Value(v) => MapResultWire::Value(v),
            MapResult::HolonId(id) => MapResultWire::HolonId(id),
            MapResult::EssentialContent(c) => MapResultWire::EssentialContent(c),
            MapResult::DanceResponse(r) => {
                MapResultWire::DanceResponse(DanceResponseWire::from(&r))
            }
        }
    }
}
