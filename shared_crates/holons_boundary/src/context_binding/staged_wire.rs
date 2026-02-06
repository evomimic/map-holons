use crate::context_binding::staged_relationship_wire::StagedRelationshipMapWire;
use base_types::MapInteger;
use core_types::{HolonError, LocalId, PropertyMap};
use holons_core::core_shared_objects::holon::{HolonState, StagedState, ValidationState};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::StagedHolon;
use serde::{Deserialize, Serialize};
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
    errors: Vec<HolonError>,
}

impl StagedHolonWire {
    pub fn bind(self, context: Arc<TransactionContext>) -> Result<StagedHolon, HolonError> {
        Ok(StagedHolon::from_wire_parts(
            self.version,
            self.holon_state,
            self.staged_state,
            self.validation_state,
            self.property_map,
            self.staged_relationships.bind(context)?,
            self.original_id,
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
            errors: value.errors().to_vec(),
        }
    }
}
