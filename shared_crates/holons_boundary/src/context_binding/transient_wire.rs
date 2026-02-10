use crate::context_binding::transient_relationship_wire::TransientRelationshipMapWire;
use base_types::MapInteger;
use core_types::{HolonError, LocalId, PropertyMap};
use holons_core::core_shared_objects::holon::{HolonState, ValidationState};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::TransientHolon;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransientHolonWire {
    version: MapInteger,
    holon_state: HolonState,
    validation_state: ValidationState,
    property_map: PropertyMap,
    transient_relationships: TransientRelationshipMapWire,
    original_id: Option<LocalId>,
}

impl TransientHolonWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<TransientHolon, HolonError> {
        Ok(TransientHolon::from_wire_parts(
            self.version,
            self.holon_state,
            self.validation_state,
            self.property_map,
            self.transient_relationships.bind(context)?,
            self.original_id,
        ))
    }
}

impl From<&TransientHolon> for TransientHolonWire {
    fn from(value: &TransientHolon) -> Self {
        Self {
            version: value.version().clone(),
            holon_state: value.holon_state().clone(),
            validation_state: value.validation_state().clone(),
            property_map: value.property_map().clone(),
            transient_relationships: TransientRelationshipMapWire::from(
                value.transient_relationships(),
            ),
            original_id: value.original_id_ref().cloned(),
        }
    }
}
