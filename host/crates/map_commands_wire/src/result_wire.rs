use base_types::BaseValue;
use core_types::HolonId;
use holons_boundary::{DanceResponseWire, HolonCollectionWire, HolonReferenceWire};
use holons_core::core_shared_objects::transactions::TxId;
use holons_core::descriptors::{Descriptor, RelationshipDirection};
use serde::{Deserialize, Serialize};

use map_commands_contract::MapResult;

/// Serializable result variants for MAP Command responses.
///
/// These represent the successful return values from command execution,
/// serialized for IPC transport back to the TypeScript client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MapResultWire {
    /// Command completed with no return value.
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
    Reference(HolonReferenceWire),

    /// Deliberate exception for duplicate-base-key staging lookup.
    ///
    /// General plural command results should prefer `Collection(HolonCollectionWire)`.
    References(Vec<HolonReferenceWire>),

    /// Canonical plural command result carrier at the IPC boundary.
    Collection(HolonCollectionWire),

    /// Ordered lifecycle-valid relationship descriptors and their directions.
    QualifiedRelationships(Vec<QualifiedRelationshipWire>),

    /// Universal scalar return.
    Value(BaseValue),

    /// Returns a holon id.
    HolonId(HolonId),

    /// Transitional dance-result exception retained at the IPC boundary.
    DanceResponse(DanceResponseWire),
}

/// Wire-safe qualified relationship discovery result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualifiedRelationshipWire {
    pub descriptor: HolonReferenceWire,
    pub direction: RelationshipDirectionWire,
}

/// Direction of a relationship descriptor relative to its declared edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RelationshipDirectionWire {
    Declared,
    Inverse,
}

impl From<RelationshipDirection> for RelationshipDirectionWire {
    fn from(direction: RelationshipDirection) -> Self {
        match direction {
            RelationshipDirection::Declared => Self::Declared,
            RelationshipDirection::Inverse => Self::Inverse,
        }
    }
}

impl From<MapResult> for MapResultWire {
    fn from(result: MapResult) -> Self {
        match result {
            MapResult::None => MapResultWire::None,
            MapResult::UndoComplete => MapResultWire::UndoComplete,
            MapResult::RedoComplete => MapResultWire::RedoComplete,
            MapResult::UndoToMarkerComplete => MapResultWire::UndoToMarkerComplete,
            MapResult::RedoToMarkerComplete => MapResultWire::RedoToMarkerComplete,
            MapResult::TransactionCreated { tx_id } => MapResultWire::TransactionCreated { tx_id },
            MapResult::Reference(r) => MapResultWire::Reference(HolonReferenceWire::from(&r)),
            MapResult::References(refs) => {
                MapResultWire::References(refs.iter().map(HolonReferenceWire::from).collect())
            }
            MapResult::Collection(c) => MapResultWire::Collection(HolonCollectionWire::from(&c)),
            MapResult::QualifiedRelationships(relationships) => {
                MapResultWire::QualifiedRelationships(
                    relationships
                        .iter()
                        .map(|relationship| QualifiedRelationshipWire {
                            descriptor: HolonReferenceWire::from(relationship.descriptor.holon()),
                            direction: relationship.direction.into(),
                        })
                        .collect(),
                )
            }
            MapResult::Value(v) => MapResultWire::Value(v),
            MapResult::HolonId(id) => MapResultWire::HolonId(id),
            MapResult::DanceResponse(r) => {
                MapResultWire::DanceResponse(DanceResponseWire::from(&r))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{QualifiedRelationshipWire, RelationshipDirectionWire};
    use holons_boundary::{HolonReferenceWire, TransientReferenceWire};
    use serde_json::{from_str, to_string};

    #[test]
    fn qualified_relationship_wire_round_trips_direction_with_its_descriptor() {
        let relationship = QualifiedRelationshipWire {
            descriptor: HolonReferenceWire::Transient(TransientReferenceWire::new(
                holons_core::core_shared_objects::transactions::TxId::from_str("7")
                    .expect("valid transaction id"),
                core_types::TemporaryId(uuid::Uuid::nil()),
            )),
            direction: RelationshipDirectionWire::Inverse,
        };

        let serialized = to_string(&relationship).expect("serialize qualified relationship");
        let decoded: QualifiedRelationshipWire =
            from_str(&serialized).expect("deserialize qualified relationship");

        assert_eq!(decoded, relationship);
    }
}
