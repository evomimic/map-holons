use crate::transient_relationship_wire::TransientRelationshipMapWire;
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
    pub fn bind(self, context: Arc<TransactionContext>) -> Result<TransientHolon, HolonError> {
        Ok(TransientHolon {
            version: self.version,
            holon_state: self.holon_state,
            validation_state: self.validation_state,
            property_map: self.property_map,
            transient_relationships: self.transient_relationships.bind(context)?,
            original_id: self.original_id,
        })
    }
}

impl From<&TransientHolon> for TransientHolonWire {
    fn from(value: &TransientHolon) -> Self {
        Self {
            version: value.version.clone(),
            holon_state: value.holon_state.clone(),
            validation_state: value.validation_state.clone(),
            property_map: value.property_map.clone(),
            transient_relationships: TransientRelationshipMapWire::from(
                &value.transient_relationships,
            ),
            original_id: value.original_id.clone(),
        }
    }
}
