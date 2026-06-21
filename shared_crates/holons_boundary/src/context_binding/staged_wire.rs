use crate::context_binding::staged_relationship_wire::StagedRelationshipMapWire;
use base_types::MapInteger;
use core_types::{HolonError, LocalId, PropertyMap, RelationshipName};
use holons_core::core_shared_objects::holon::{HolonState, StagedState, ValidationState};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::StagedHolon;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StagedHolonWire {
    version: MapInteger,
    holon_state: HolonState,
    staged_state: StagedState,
    validation_state: ValidationState,
    property_map: PropertyMap,
    staged_relationships: StagedRelationshipMapWire,
    original_id: Option<LocalId>,
    #[serde(default)]
    versioned_source_id: Option<LocalId>,
    #[serde(default)]
    touched_relationship_names: BTreeSet<RelationshipName>,
    errors: Vec<HolonError>,
}

impl StagedHolonWire {
    /// Binds this holon's nested references to a TransactionContext, validating tx_id.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<StagedHolon, HolonError> {
        Ok(StagedHolon::from_parts(
            self.version,
            self.holon_state,
            self.staged_state,
            self.validation_state,
            self.property_map,
            self.staged_relationships.bind(context)?,
            self.original_id,
            self.versioned_source_id,
            self.touched_relationship_names,
            self.errors,
        ))
    }

    /// Rebinds this holon's nested references to a different transaction
    /// context, bypassing tx_id validation. See [`StagedRelationshipMapWire::rebind`].
    pub fn rebind(self, context: &Arc<TransactionContext>) -> Result<StagedHolon, HolonError> {
        Ok(StagedHolon::from_parts(
            self.version,
            self.holon_state,
            self.staged_state,
            self.validation_state,
            self.property_map,
            self.staged_relationships.rebind(context)?,
            self.original_id,
            self.versioned_source_id,
            self.touched_relationship_names,
            self.errors,
        ))
    }
}

impl From<&StagedHolon> for StagedHolonWire {
    fn from(value: &StagedHolon) -> Self {
        Self {
            version: value.version().clone(),
            holon_state: value.holon_state().clone(),
            staged_state: value.staged_state().clone(),
            validation_state: value.validation_state().clone(),
            property_map: value.property_map().clone(),
            staged_relationships: StagedRelationshipMapWire::from(value.staged_relationships()),
            original_id: value.original_id_ref().cloned(),
            versioned_source_id: value.versioned_source_id_ref().cloned(),
            touched_relationship_names: value.touched_relationship_names().clone(),
            errors: value.errors().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_types::MapString;
    use core_types::RelationshipName;
    use holons_core::core_shared_objects::WriteableHolonState;

    /// Regression guard (issue #515): relationship-mutation intent
    /// (`touched_relationship_names`) is transaction-scoped staged state that must
    /// survive the dance / session-state round-trip. Sweetests export staged holons
    /// to `StagedHolonWire` between the `add` and `commit` dances; if the wire drops
    /// the touched set, a graph-only commit rehydrates with an empty filter and
    /// persists none of the mutated relationships.
    ///
    /// This asserts the wire carries the touched set. It fails until the wire type
    /// and `From<&StagedHolon>` are taught to propagate `touched_relationship_names`.
    #[test]
    fn staged_holon_wire_preserves_touched_relationship_names() {
        let mut staged = StagedHolon::new_for_create();
        let relationship = RelationshipName(MapString("Properties".to_string()));
        staged
            .add_related_holons(relationship, Vec::new())
            .expect("recording a relationship mutation should succeed");

        let wire = StagedHolonWire::from(&staged);
        let json = serde_json::to_value(&wire).expect("StagedHolonWire should serialize");

        assert!(
            json.get("touched_relationship_names").is_some(),
            "StagedHolonWire must carry touched_relationship_names so relationship-mutation \
             intent survives the dance/session-state round-trip; without it, graph-only \
             commits lose their touched set and persist no relationship changes"
        );
    }
}
