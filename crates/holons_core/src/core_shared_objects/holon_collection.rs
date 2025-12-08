use core::fmt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::slice::{Iter, IterMut};
use std::vec::IntoIter;
use tracing::{debug, warn};

use super::holon::state::AccessType;
use crate::reference_layer::{HolonReference, HolonsContextBehavior, ReadableHolon};
use crate::HolonCollectionApi;
use base_types::{MapInteger, MapString};
use core_types::HolonError;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum CollectionState {
    Fetched,   // links have been fetched from the persistent store for this collection
    Transient, // this is the target of a transient relationship and no links should be created
    Staged,    // the links for this collection have not been persisted
    Saved,     // a staged collection for which SmartLinks have been successfully committed
    Abandoned, // a previously staged collection that was abandoned prior to being committed
}

impl fmt::Display for CollectionState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CollectionState::Fetched => write!(f, "Fetched"),
            CollectionState::Transient => write!(f, "Transient"),
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

    pub fn new_existing() -> Self {
        HolonCollection {
            state: CollectionState::Fetched,
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }

    pub fn new_saved() -> Self {
        HolonCollection {
            state: CollectionState::Saved,
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }

    pub fn new_staged() -> Self {
        HolonCollection {
            state: CollectionState::Staged,
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }

    pub fn new_transient() -> Self {
        HolonCollection {
            state: CollectionState::Transient,
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }

    pub fn clone_for_new_source(&self) -> Result<Self, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let mut collection = self.clone();
        collection.state = CollectionState::Transient;

        Ok(collection)
    }

    /// Does not retain members that are TransientReference.
    pub fn clone_for_staged(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Self, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let mut collection = HolonCollection::new_staged();
        let mut index = 0;
        for holon_reference in self.members.clone() {
            if !holon_reference.is_transient() {
                if let Some(key) = holon_reference.key(context)? {
                    collection.keyed_index.insert(key, index);
                    index += 1;
                }
                collection.members.push(holon_reference);
            }
        }

        Ok(collection)
    }

    // METHODS //

    pub fn from_parts(state: CollectionState, members: Vec<HolonReference>) -> Self {
        let keyed_index = BTreeMap::new();

        // TODO: This method should reconstitute the keyed_index from members -- but needs member.key to not require context first.
        // for (index, member) in members.iter().enumerate() {
        //     if let Some(key) = member.key() {
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
                AccessType::Read | AccessType::Write | AccessType::Commit => Ok(()), // Write access to cached Holons are ok, Commit is a no op
                AccessType::Abandon | AccessType::Clone => Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    format!("{:?}", self.state),
                )),
            },
            CollectionState::Transient => match access_type {
                AccessType::Read | AccessType::Write | AccessType::Clone => Ok(()), // Write access to cached Holons are ok, Commit is a no op
                AccessType::Abandon | AccessType::Commit => Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    format!("{:?}", self.state),
                )),
            },
            CollectionState::Staged => match access_type {
                AccessType::Abandon
                | AccessType::Clone
                | AccessType::Commit
                | AccessType::Read
                | AccessType::Write => Ok(()),
            },
            CollectionState::Saved => match access_type {
                AccessType::Commit | AccessType::Read => Ok(()),
                AccessType::Clone | AccessType::Write | AccessType::Abandon => {
                    Err(HolonError::NotAccessible(
                        format!("{:?}", access_type),
                        format!("{:?}", self.state),
                    ))
                }
            },
            CollectionState::Abandoned => match access_type {
                AccessType::Abandon | AccessType::Commit => Ok(()),
                AccessType::Clone | AccessType::Read | AccessType::Write => {
                    Err(HolonError::NotAccessible(
                        format!("{:?}", access_type),
                        format!("{:?}", self.state),
                    ))
                }
            },
        }
    }
    pub fn mark_as_staged(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        self.state = CollectionState::Staged;

        Ok(())
    }

    pub fn keyed_index(&self) -> BTreeMap<MapString, usize> {
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
}

impl HolonCollectionApi for HolonCollection {
    /// Adds the supplied HolonReferences to this holon collection and updates the keyed_index
    /// accordingly. Currently, this method requires a `context`. Use `add_reference_with_key()` to
    /// add individual references without requiring `context` when the key is known.
    fn add_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        for holon_ref in holons {
            // Add reference to collection
            self.members.push(holon_ref.clone());

            // Add reference to keyed index (unless it is a duplicate key, in which case just
            // issue a warning
            let key = holon_ref.key(context)?;

            if let Some(key) = key {
                if let Some(&_index) = self.keyed_index.get(&key) {
                    // let existing_holon_ref = &self.members[index];
                    warn!("Duplicate holons with key {:#?}", key.0.clone());
                } else {
                    let index = self.members.len() - 1;
                    // self.members.push(holon_ref.clone());
                    self.keyed_index.insert(key, index);
                }
            }
        }

        Ok(())
    }

    /// Adds the supplied HolonReference to this holon collection and updates the keyed_index
    /// according to the supplied key. This allows the collection to be populated when key is
    /// known and context may not be available.
    fn add_reference_with_key(
        &mut self,
        key: Option<&MapString>,
        reference: &HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        let index = self.members.len() - 1;
        self.members.push(reference.clone());
        if let Some(key) = key {
            self.keyed_index.insert(key.clone(), index);
        }
        Ok(())
    }

    fn get_count(&self) -> MapInteger {
        MapInteger(self.members.len() as i64)
    }

    fn get_by_index(&self, index: usize) -> Result<HolonReference, HolonError> {
        if index < self.members.len() {
            Ok(self.members[index].clone())
        } else {
            Err(HolonError::IndexOutOfRange(format!("Index {} is out of bounds", index)))
        }
    }

    fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let index: Option<&usize> = self.keyed_index.get(key);
        debug!("Found {:?} at index: {:?}", key, index);
        if let Some(index) = index {
            Ok(Some(self.members[*index].clone()))
        } else {
            Ok(None)
        }
    }

    fn remove_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        for holon in holons {
            self.members.retain(|x| x != &holon);
            if let Some(key) = holon.key(context)? {
                self.keyed_index.remove(&key);
            }
        }
        // adjust new order of members in the keyed_index
        let mut i = 0;
        for member in self.members.clone() {
            if let Some(key) = member.key(context)? {
                self.keyed_index.insert(key, i);
                i += 1;
            }
        }

        Ok(())
    }
}
// Owned iteration
impl IntoIterator for HolonCollection {
    type Item = HolonReference;
    type IntoIter = IntoIter<HolonReference>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.into_iter()
    }
}

// Iteration by reference
impl<'a> IntoIterator for &'a HolonCollection {
    type Item = &'a HolonReference;
    type IntoIter = Iter<'a, HolonReference>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.iter()
    }
}

// Iteration by mutable reference
impl<'a> IntoIterator for &'a mut HolonCollection {
    type Item = &'a mut HolonReference;
    type IntoIter = IterMut<'a, HolonReference>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.iter_mut()
    }
}
