use std::collections::BTreeMap;

use hdk::prelude::*;

use shared_types_holon::MapString;

use crate::context::HolonsContext;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonGettable;
use crate::holon_reference::HolonReference;

/// These keyed collections can be used when there is a need for a collection of Holons, which we
/// don't intend to persist and which are independent of a relationship. They currently contain
/// references to staged or existing holons.
///

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TransientCollection {
    // TransientCollections do not undergo state transitions, so they don't need a state field
    // or a guard function.
    // NOTE: We may need to wrap HolonReference in a new TransientReference enum if transient holons
    // are implemented and allowed as members of TransientCollection.
    members: Vec<HolonReference>,
    keyed_index: BTreeMap<MapString, usize>, // usize is an index into the members vector
}

impl TransientCollection {
    pub fn new() -> Self {
        TransientCollection {
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }

    pub fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError> {
        let index = self.keyed_index.get(key);
        if let Some(index) = index {
            Ok(Some(self.members[*index].clone()))
        } else {
            Ok(None)
        }
    }

    pub fn add_reference(
        &mut self,
        context: &HolonsContext,
        holon_ref: HolonReference,
    ) -> Result<(), HolonError> {
        let key = holon_ref.get_key(context)?;

        if let Some(key) = key {
            if let Some(&index) = self.keyed_index.get(&key) {
                // let existing_holon_ref = &self.members[index];
                warn!("Duplicate holons with key {:#?}", key.0.clone());
            } else {
                let index = self.members.len();
                self.members.push(holon_ref.clone());
                self.keyed_index.insert(key, index);
            }
        }
        Ok(())
    }

    pub fn add_references(
        &mut self,
        context: &HolonsContext,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        for holon_ref in holons {
            self.add_reference(context, holon_ref)?;
        }
        Ok(())
    }
}
