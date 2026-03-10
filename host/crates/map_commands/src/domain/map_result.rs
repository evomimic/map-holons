use base_types::BaseValue;
use holons_core::core_shared_objects::transactions::TxId;
use holons_core::core_shared_objects::{Holon, HolonCollection};
use holons_core::dances::DanceResponse;
use holons_core::query_layer::NodeCollection;
use holons_core::reference_layer::{HolonReference, TransientReference};

/// Domain-level result variants from command execution.
///
/// These are runtime types containing bound references. They are
/// converted to `MapResultWire` before crossing the IPC boundary.
#[derive(Debug)]
pub enum MapResult {
    /// Command completed with no return value.
    Unit,

    /// Returns a new transaction id (from BeginTransaction).
    TransactionCreated { tx_id: TxId },

    /// Returns a committed transaction result.
    Committed,

    /// Returns a transient reference (from CreateTransientHolon).
    TransientReference(TransientReference),

    /// Returns a holon reference.
    HolonReference(HolonReference),

    /// Returns a single holon.
    Holon(Holon),

    /// Returns a collection of holons.
    HolonCollection(HolonCollection),

    /// Returns a node collection (query result).
    NodeCollection(NodeCollection),

    /// Returns a single property value.
    PropertyValue(Option<BaseValue>),

    /// Returns a dance response.
    DanceResponse(DanceResponse),
}
