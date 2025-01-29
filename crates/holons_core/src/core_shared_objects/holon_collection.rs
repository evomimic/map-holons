use crate::reference_layer::{HolonReadable, HolonReference, HolonsContextBehavior};

use crate::core_shared_objects::{AccessType, HolonError};
use crate::{HolonCollectionApi, HolonSpaceBehavior};
use core::fmt;
use hdk::prelude::*;
use shared_types_holon::{MapInteger, MapString};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum CollectionState {
    Fetched,   // links have been fetched from the persistent store for this collection
    Staged,    // the links for this collection have not been persisted
    Saved,     // a staged collection for which SmartLinks have been successfully committed
    Abandoned, // a previously staged collection that was abandoned prior to being committed
}

impl fmt::Display for CollectionState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CollectionState::Fetched => write!(f, "Fetched"),
            CollectionState::Staged => write!(f, "Staged"),
            CollectionState::Saved => write!(f, "Saved"),
            CollectionState::Abandoned => write!(f, "Abandoned"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct HolonCollection {
    state: CollectionState,
    members: Vec<HolonReference>,
    keyed_index: BTreeMap<MapString, usize>, // usize is an index into the members vector
}

impl HolonCollection {
    // CONSTRUCTORS //

    pub fn new_staged() -> Self {
        HolonCollection {
            state: CollectionState::Staged,
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }
    pub fn new_existing() -> Self {
        HolonCollection {
            state: CollectionState::Fetched,
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }

    pub fn clone_for_new_source(&self) -> Result<Self, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let mut collection = self.clone();
        collection.state = CollectionState::Staged;

        Ok(collection)
    }

    // METHODS //

    pub fn from_parts(state: CollectionState, members: Vec<HolonReference>) -> Self {
        let keyed_index = BTreeMap::new();

        // TODO: This method should reconstitute the keyed_index from members -- but needs member.get_key to not require context first.
        // for (index, member) in members.iter().enumerate() {
        //     if let Some(key) = member.get_key() {
        //         keyed_index.insert(key, index);
        //     }
        // }
        HolonCollection { state, members, keyed_index }
    }
    /// Checks if requested `access_type` is acceptable given the collection's current `state`.
    /// If not, returns `NotAccessible` error
    pub fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self.state {
            CollectionState::Fetched => match access_type {
                AccessType::Read | AccessType::Write => Ok(()), // Write access to cached Holons are ok
                AccessType::Abandon | AccessType::Commit => Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    format!("{:?}", self.state),
                )),
            },
            CollectionState::Staged => match access_type {
                AccessType::Read | AccessType::Write | AccessType::Abandon | AccessType::Commit => {
                    Ok(())
                }
            },
            CollectionState::Saved => match access_type {
                AccessType::Read | AccessType::Commit => Ok(()),
                AccessType::Write | AccessType::Abandon => Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    format!("{:?}", self.state),
                )),
            },
            CollectionState::Abandoned => match access_type {
                AccessType::Commit | AccessType::Abandon => Ok(()),
                AccessType::Read | AccessType::Write => Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    format!("{:?}", self.state),
                )),
            },
        }
    }
    pub fn mark_as_staged(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        self.state = CollectionState::Staged;

        Ok(())
    }

    pub fn get_by_index(&self, index: usize) -> Result<HolonReference, HolonError> {
        if index < self.members.len() {
            Ok(self.members[index].clone())
        } else {
            Err(HolonError::IndexOutOfRange(format!("Index {} is out of bounds", index)))
        }
    }

    pub fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let index = self.keyed_index.get(key);
        if let Some(index) = index {
            Ok(Some(self.members[*index].clone()))
        } else {
            Ok(None)
        }
    }

    pub fn get_count(&self) -> MapInteger {
        MapInteger(self.members.len() as i64)
    }

    pub fn get_keyed_index(&self) -> BTreeMap<MapString, usize> {
        self.keyed_index.clone()
    }

    /// Returns the current state of the HolonCollection.
    ///
    /// # Semantics
    /// The state indicates the lifecycle stage of the collection, such as whether it has been fetched
    /// from the persistent store, staged for changes, saved after committing changes, or abandoned.
    ///
    /// # Usage
    /// Use this method to inspect the current state of the collection. DO NOT use this method to
    /// make decisions about whether certain operations (e.g., reading, writing, committing) are
    /// permissible. Use `is_accessible()` for this purpose instead.
    pub fn get_state(&self) -> CollectionState {
        self.state.clone()
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

    /// Adds the supplied HolonReferences to this holon collection and updates the keyed_index
    /// accordingly. Currently, this method requires a `context`. Use `add_reference_with_key()` to
    /// add individual references without requiring `context` when the key is known.
    pub fn add_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        for holon_ref in holons {
            self.add_reference(context, holon_ref)?;
        }

        Ok(())
    }

    pub fn remove_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

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

    /// Adds the supplied HolonReference to this holon collection and updates the keyed_index
    /// according to the supplied key. This allows the collection to be populated when key is
    /// known and context may not be available.
    pub fn add_reference_with_key(
        &mut self,
        key: Option<&MapString>,
        reference: &HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        let index = self.members.len();
        self.members.push(reference.clone());
        if let Some(key) = key {
            self.keyed_index.insert(key.clone(), index);
        }
        Ok(())
    }
}

impl HolonCollectionApi for HolonCollection {
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
