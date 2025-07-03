use serde::{Serialize, Deserialize};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt,
    rc::Rc,
};

use super::{ReadableRelationship, TransientRelationshipMap};
use crate::core_shared_objects::HolonCollection;
use base_types::MapString;
use core_types::HolonError;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct RelationshipName(pub MapString);
impl fmt::Display for RelationshipName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
/// Custom RelationshipMap is only used for caching and will never be serialized
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipMap {
    map: RefCell<HashMap<RelationshipName, Rc<HolonCollection>>>,
}
impl RelationshipMap {
    /// Creates a new, empty `RelationshipMap`.
    pub fn new_empty() -> Self {
        Self { map: RefCell::new(HashMap::new()) }
    }

    /// Returns a shared reference (`Rc<HolonCollection>`) for the given `relationship_name`.
    /// Returns `None` if the relationship is not found.
    pub fn get_collection_for_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Option<Rc<HolonCollection>> {
        // Borrow the map immutably and clone the Rc for the requested relationship
        self.map.borrow().get(relationship_name).cloned()
    }
    /// Inserts a `HolonCollection` into the `RelationshipMap` for the given `relationship_name`.
    pub fn insert(&self, relationship_name: RelationshipName, collection: Rc<HolonCollection>) {
        // Borrow the map mutably and insert the new collection
        self.map.borrow_mut().insert(relationship_name, collection);
    }

    /// Iterates over all relationships in the `RelationshipMap`.
    /// Returns a vector of `(RelationshipName, Rc<HolonCollection>)` pairs for read-only access.
    pub fn iter(&self) -> Vec<(RelationshipName, Rc<HolonCollection>)> {
        self.map.borrow().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

impl ReadableRelationship for RelationshipMap {
    // =====================
    //     CONSTRUCTORS
    // =====================

    fn clone_for_new_source(&self) -> Result<TransientRelationshipMap, HolonError> {
        let mut cloned_relationship_map = BTreeMap::new();

        for (name, collection) in self.map.borrow().iter() {
            let cloned_collection = collection.clone_for_new_source()?; // Assumes `clone_for_new_source` exists on `HolonCollection`.
            cloned_relationship_map.insert(name.clone(), Rc::new(RefCell::new(cloned_collection)));
        }

        Ok(TransientRelationshipMap::new(cloned_relationship_map))
    }

    // ====================
    //    DATA ACCESSORS
    // ====================

    fn get_related_holons(&self, relationship_name: &RelationshipName) -> Rc<HolonCollection> {
        if let Some(rc_collection) = self.map.borrow().get(relationship_name) {
            // Clone the inner HolonCollection
            Rc::clone(rc_collection)
        } else {
            // Return a new Rc<HolonCollection> if the entry doesn't exist
            Rc::new(HolonCollection::new_staged())
        }
    }
}

// Implement Serialize for RelationshipMap
impl Serialize for RelationshipMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let borrowed_map = self.map.borrow();
        let serializable_map: HashMap<_, _> = borrowed_map
            .iter()
            .map(|(key, value)| (key.clone(), &**value)) // Deref Rc
            .collect();
        serializable_map.serialize(serializer)
    }
}

// Implement Deserialize for RelationshipMap
impl<'de> Deserialize<'de> for RelationshipMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let deserialized_map: HashMap<RelationshipName, HolonCollection> =
            HashMap::deserialize(deserializer)?;
        let wrapped_map: HashMap<_, _> =
            deserialized_map.into_iter().map(|(key, value)| (key, Rc::new(value))).collect();
        Ok(RelationshipMap { map: RefCell::new(wrapped_map) })
    }
}
