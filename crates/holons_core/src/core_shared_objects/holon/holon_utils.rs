use std::fmt;

use derive_new::new;
use hdi::prelude::{Record,RecordEntry};
use serde::{Deserialize, Serialize};
use shared_types_holon::{HolonNode, MapString, PropertyMap};

use crate::HolonError;

use super::{state::{HolonState, ValidationState}, HolonBehavior};

//  ================
//   HELPER OBJECTS
//  ================


/// Used for testing in order to match the EssentialContent of a Holon.
#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct EssentialHolonContent {
    pub property_map: PropertyMap,
    // pub relationship_map: RelationshipMap,
    pub key: Option<MapString>,
    pub errors: Vec<HolonError>,
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


//  ===========
//   FUNCTIONS
//  ===========

pub fn key_info(holon: &impl HolonBehavior) -> String {
    match holon.get_key() {
        Ok(Some(key)) => format!("key: {}", key.0),
        Ok(None) => "key: <None>".to_string(),
        Err(_) => "key: <Error>".to_string(),
    }
}

pub fn local_id_info(holon: &impl HolonBehavior) -> String {
    match holon.get_local_id() {
        Ok(local_id) => format!("local_id: {}", local_id.0),
        Err(_) => "local_id: <Error>".to_string(),
    }
}