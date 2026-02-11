use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

use base_types::MapString;
use core_types::{HolonError, PropertyMap, RelationshipName};

use crate::{
    core_shared_objects::TransientRelationshipMap, CollectionState, HolonCollection,
    HolonCollectionApi, HolonReference, RelationshipMap, StagedRelationshipMap,
};

use super::state::{HolonState, ValidationState};

//  ================
//   HELPER OBJECTS
//  ================

/// Used for testing in order to match the EssentialContent of a Holon.
#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct EssentialHolonContent {
    pub property_map: PropertyMap,
    pub key: Option<MapString>,
    pub errors: Vec<HolonError>,
}

// ==== TESTING PURPOSES ==== //

#[derive(new, Clone, Debug, Eq, PartialEq, Default)]
pub struct EssentialRelationshipMap {
    map: BTreeMap<RelationshipName, HolonCollection>,
}

impl EssentialRelationshipMap {
    pub fn add_related_holons(
        &mut self,
        collection_state: CollectionState,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        let collection = match collection_state {
            CollectionState::Transient => {
                if holons.iter().any(|hr| !hr.is_transient()) {
                    return Err(HolonError::InvalidParameter(
                        "Holons to be added are not all Transient".to_string(),
                    ));
                } else {
                    self.map.entry(relationship_name).or_insert(HolonCollection::new_transient())
                }
            }
            CollectionState::Staged => {
                if holons.iter().any(|hr| !hr.is_staged()) {
                    return Err(HolonError::InvalidParameter(
                        "Holons to be added are not all Staged".to_string(),
                    ));
                } else {
                    self.map.entry(relationship_name).or_insert(HolonCollection::new_staged())
                }
            }
            CollectionState::Saved => {
                if holons.iter().any(|hr| !hr.is_saved()) {
                    return Err(HolonError::InvalidParameter(
                        "Holons to be added are not all Saved".to_string(),
                    ));
                } else {
                    self.map.entry(relationship_name).or_insert(HolonCollection::new_saved())
                }
            }
            _ => {
                return Err(HolonError::NotImplemented(
                    "Abandoned or Fetched not yet implemented".to_string(),
                ))
            }
        };

        collection.add_references(holons)?;

        Ok(())
    }

    pub fn remove_related_holons(
        &mut self,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        if let Some(collection) = self.map.get_mut(relationship_name) {
            collection.remove_references(holons)?;

            Ok(())
        } else {
            Err(HolonError::InvalidRelationship(
                format!("Invalid relationship: {}", relationship_name),
                "No matching collection found in map".to_string(),
            ))
        }
    }
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
            essential.insert(name.clone(), collection.clone());
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
