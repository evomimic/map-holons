use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

use base_types::MapString;
use core_types::{HolonError, PropertyMap, RelationshipName};

use crate::{
    core_shared_objects::TransientRelationshipMap, HolonCollection, RelationshipMap,
    StagedRelationshipMap,
};

use super::state::{HolonState, ValidationState};

//  ================
//   HELPER OBJECTS
//  ================

/// Used for testing in order to match the EssentialContent of a Holon.
#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct EssentialHolonContent {
    pub property_map: PropertyMap,
    pub relationships: EssentialRelationshipMap,
    pub key: Option<MapString>,
    pub errors: Vec<HolonError>,
}

// ==== TESTING PURPOSES ==== //

#[derive(new, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct EssentialRelationshipMap {
    map: BTreeMap<RelationshipName, HolonCollection>,
}

impl From<RelationshipMap> for EssentialRelationshipMap {
    fn from(map: RelationshipMap) -> Self {
        let mut essential = BTreeMap::new();

        for (name, arc_lock) in map.iter() {
            let collection = arc_lock.read().unwrap();
            essential.insert(name.clone(), collection.clone());
        }
        Self::new(essential)
    }
}

impl From<TransientRelationshipMap> for EssentialRelationshipMap {
    fn from(map: TransientRelationshipMap) -> Self {
        let mut essential = BTreeMap::new();

        for (name, arc_lock) in map.iter() {
            let collection = arc_lock.read().unwrap();
            essential.insert(name.clone(), collection.clone());
        }
        Self::new(essential)
    }
}

impl From<StagedRelationshipMap> for EssentialRelationshipMap {
    fn from(map: StagedRelationshipMap) -> Self {
        let mut essential = BTreeMap::new();

        for (name, arc_lock) in map.iter() {
            let collection = arc_lock.read().unwrap();
            // TODO: figure out how best to change this conversion to CollectionState::Transient so that this implementation is friendly
            // to purposes outside of testing.
            essential.insert(name.clone(), collection.clone_for_new_source().unwrap());
        }
        Self::new(essential)
    }
}

#[derive(Debug, Clone)]
pub struct HolonSummary {
    pub key: Option<String>,
    pub local_id: Option<String>,
    pub state: HolonState,
    pub validation_state: ValidationState,
}

impl fmt::Display for HolonSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "HolonSummary {{ key: {:?}, local_id: {:?}, state: {}, validation_state: {:?} }}",
            self.key, self.local_id, self.state, self.validation_state,
        )
    }
}
