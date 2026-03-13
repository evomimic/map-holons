use base_types::{BaseValue, MapString};
use core_types::HolonId;
use holons_core::core_shared_objects::holon::EssentialHolonContent;
use holons_core::core_shared_objects::transactions::TxId;
use holons_core::core_shared_objects::HolonCollection;
use holons_core::dances::DanceResponse;
use holons_core::query_layer::NodeCollection;
use holons_core::reference_layer::HolonReference;

/// Domain-level result variants from command execution.
///
/// These are runtime types containing bound references. They are
/// converted to `MapResultWire` before crossing the IPC boundary.
#[derive(Debug)]
pub enum MapResult {
    /// Command completed with no return value.
    None,

    /// Returns a new transaction id (from BeginTransaction).
    TransactionCreated { tx_id: TxId },

    /// Returns a committed transaction result.
    CommitResponse(HolonReference),

    /// Returns a holon reference.
    HolonReference(HolonReference),

    /// Returns a collection of holon references.
    HolonReferences(Vec<HolonReference>),

    /// Returns an indexed collection of holons.
    HolonCollection(HolonCollection),

    /// Returns a node collection (query result).
    NodeCollection(NodeCollection),

    /// Returns a single property value.
    PropertyValue(Option<BaseValue>),

    /// Returns a string value (e.g. from `versioned key()`).
    StringValue(MapString),

    /// Returns a holon id.
    HolonId(HolonId),

    /// Returns the essential content of a holon.
    EssentialContent(EssentialHolonContent),

    /// Returns a dance response.
    DanceResponse(DanceResponse),
}
