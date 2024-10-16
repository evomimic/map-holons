use std::collections::BTreeMap;
use hdi::prelude::*;


use shared_types_holon::{PropertyValue, SavedPropertyMap};
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct HolonPropertyMap(pub BTreeMap<HolonReference, PropertyValue>);

impl HolonPropertyMap {

    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    /// This function constructs a HolonPropertyMap from a SavedPropertyMap
    ///
    pub fn from_saved_map(saved_map: &SavedPropertyMap) -> Self {
        let map = saved_map.iter().map(|(holon_id, property_value)| {
            let holon_reference = HolonReference::from_holon_id(holon_id.clone());
            (holon_reference, property_value.clone())
        }).collect();
        HolonPropertyMap(map)
    }

    // Converts a HolonPropertyMap into a SavedPropertyMap
    pub fn to_saved_map(&self) -> Result<SavedPropertyMap, HolonError> {
        self.0.iter().try_fold(BTreeMap::new(), |mut acc,
                                                 (holon_reference, property_value)| {
            match holon_reference {
                HolonReference::Smart(smart_ref) => {
                    let holon_id = smart_ref.get_holon_id_no_context();
                    acc.insert(holon_id, property_value.clone());
                    Ok(acc)
                },
                HolonReference::Staged(_) => {
                    Err(HolonError::InvalidHolonReference("Expected only SmartReferences in the\
                        HolonPropertyMap, but found a StagedReference instead".to_string()))
                }
            }
        })
    }
}

