use base_types::BaseValue;
use core_types::HolonId;
use holons_core::core_shared_objects::transactions::TxId;
use holons_core::core_shared_objects::HolonCollection;
use holons_core::dances::DanceResponse;
use holons_core::descriptors::{QualifiedRelationship, RelationshipDescriptor};
use holons_core::reference_layer::HolonReference;
use std::fmt;

/// A relationship descriptor together with its effective traversal direction.
pub struct QualifiedRelationshipResult {
    pub descriptor: RelationshipDescriptor,
    pub direction: holons_core::descriptors::RelationshipDirection,
}

impl fmt::Debug for QualifiedRelationshipResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedRelationshipResult")
            .field("direction", &self.direction)
            .finish_non_exhaustive()
    }
}

impl From<QualifiedRelationship> for QualifiedRelationshipResult {
    fn from(relationship: QualifiedRelationship) -> Self {
        Self { descriptor: relationship.descriptor, direction: relationship.descriptor_direction }
    }
}

/// Domain-level result variants from command execution.
///
/// These are runtime types containing bound references. They are
/// converted to `MapResultWire` before crossing the IPC boundary.
#[derive(Debug)]
pub enum MapResult {
    /// Command completed with no return value (also used for "not found").
    None,

    /// Command completed an undo operation.
    UndoComplete,

    /// Command completed a redo operation.
    RedoComplete,

    /// Command completed an undo to marker operation.
    UndoToMarkerComplete,

    /// Command completed a redo to marker operation.
    RedoToMarkerComplete,

    /// Returns a new transaction id (from BeginTransaction).
    TransactionCreated { tx_id: TxId },

    /// Returns a holon reference.
    Reference(HolonReference),

    /// Deliberate exception for duplicate-base-key staging lookup.
    ///
    /// General plural command results should prefer `Collection(HolonCollection)`.
    References(Vec<HolonReference>),

    /// Canonical plural command result carrier.
    Collection(HolonCollection),

    /// Ordered lifecycle-valid relationship descriptors with their directions.
    QualifiedRelationships(Vec<QualifiedRelationshipResult>),

    /// Universal scalar return — covers MapString, MapInteger, MapBoolean, PropertyValue.
    Value(BaseValue),

    /// Returns a holon id.
    HolonId(HolonId),

    /// Transitional dance-result exception retained for legacy and in-flight dance paths.
    DanceResponse(DanceResponse),
}
