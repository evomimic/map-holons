use std::fmt;

use base_types::MapString;
use derive_new::new;
use integrity_core_types::PropertyMap;
use serde::{Deserialize, Serialize};

use crate::HolonError;

use super::state::{HolonState, ValidationState};

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
