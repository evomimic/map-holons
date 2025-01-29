use std::collections::BTreeMap;

use hdk::prelude::*;

use crate::reference_layer::{
    HolonReadable, HolonReference, HolonsContextBehavior, TransientCollectionBehavior,
};

use crate::core_shared_objects::HolonError;
use crate::HolonCollectionApi;
use shared_types_holon::{MapInteger, MapString};

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
        TransientCollection { members: Vec::new(), keyed_index: BTreeMap::new() }
    }

    fn add_reference(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holon_ref: HolonReference,
    ) -> Result<(), HolonError> {
        let key = holon_ref.get_key(context)?;

        if let Some(key) = key {
            if let Some(&_index) = self.keyed_index.get(&key) {
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

    /// Adds the supplied HolonReferences to this transient collection and updates the keyed_index
    /// accordingly. Currently, this method requires a `context`. Use `add_reference_with_key()` to
    /// add individual references without requiring `context` when the key is known.
    fn add_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        for holon_ref in holons {
            self.add_reference(context, holon_ref)?;
        }
        Ok(())
    }

    fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError> {
        let index = self.keyed_index.get(key);
        if let Some(index) = index {
            Ok(Some(self.members[*index].clone()))
        } else {
            Ok(None)
        }
    }

    /// Returns a reference to the vector of HolonReference members in the collection.
    ///
    /// # Semantics
    /// The members represent individual holons that are part of this collection. Each member is a
    /// reference to a Holon, which can be either staged or saved.
    ///
    /// # Usage
    /// Use this method for read-only access to the members of this collection for iteration,
    /// inspection, or performing bulk operations. This method does not clone the members,
    /// thus avoiding unnecessary copying.
    pub fn get_members(&self) -> &Vec<HolonReference> {
        &self.members
    }

    pub fn remove_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        for holon in holons {
            self.members.retain(|x| x != &holon);
            if let Some(key) = holon.get_key(context)? {
                self.keyed_index.remove(&key);
            }
        }
        // adjust new order of members in the keyed_index
        let mut i = 0;
        for member in self.members.clone() {
            if let Some(key) = member.get_key(context)? {
                self.keyed_index.insert(key, i);
                i += 1;
            }
        }

        Ok(())
    }
}

impl HolonCollectionApi for TransientCollection {
    fn add_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        Ok(self.add_references(context, holons)?)
    }

    fn add_reference_with_key(
        &mut self,
        key: Option<&MapString>,
        reference: &HolonReference,
    ) -> Result<(), HolonError> {
        Ok(self.add_reference_with_key(key, reference)?)
    }

    fn get_by_index(&self, index: usize) -> Result<HolonReference, HolonError> {
        Ok(self.get_by_index(index)?)
    }

    fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError> {
        Ok(self.get_by_key(key)?)
    }

    fn get_count(&self) -> MapInteger {
        self.get_count()
    }

    fn remove_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        Ok(self.remove_references(context, holons)?)
    }
}
