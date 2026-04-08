use base_types::BaseValue;
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
    /// Command completed with no return value (also used for "not found").
    None,

    /// Returns a new transaction id (from BeginTransaction).
    TransactionCreated { tx_id: TxId },

    /// Returns a holon reference.
    Reference(HolonReference),

    /// Returns a collection of holon references.
    References(Vec<HolonReference>),

    /// Returns an indexed collection of holons.
    Collection(HolonCollection),

    /// Returns a node collection (query result).
    NodeCollection(NodeCollection),

    /// Universal scalar return — covers MapString, MapInteger, MapBoolean, PropertyValue.
    Value(BaseValue),

    /// Returns a holon id.
    HolonId(HolonId),

    /// Returns the essential content of a holon.
    EssentialContent(EssentialHolonContent),

    /// Returns a dance response.
    DanceResponse(DanceResponse),
}
