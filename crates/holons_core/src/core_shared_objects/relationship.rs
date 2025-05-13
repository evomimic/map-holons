use crate::core_shared_objects::HolonCollection;
use hdk::prelude::*;
use base_types::MapString;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct RelationshipName(pub MapString);
impl fmt::Display for RelationshipName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
/// Custom RelationshipMap is only used for caching and will never be serialized
#[derive(SerializedBytes, Clone, Debug)]
pub struct RelationshipMap {
    map: RefCell<HashMap<RelationshipName, Rc<HolonCollection>>>,
}
impl RelationshipMap {
    /// Creates a new, empty `RelationshipMap`.
    pub fn new() -> Self {
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
