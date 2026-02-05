use crate::holon_collection_wire::HolonCollectionWire;
use core_types::{HolonError, RelationshipName};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::TransientRelationshipMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

// This wire type is required because dance RequestBody and ResponseBody may be of type Holon(Holon)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransientRelationshipMapWire {
    pub map: BTreeMap<RelationshipName, HolonCollectionWire>,
}

impl TransientRelationshipMapWire {
    pub fn bind(
        self,
        context: Arc<TransactionContext>,
    ) -> Result<TransientRelationshipMap, HolonError> {
        let mut map = BTreeMap::new();

        for (name, collection_wire) in self.map {
            let collection = collection_wire.bind(Arc::clone(&context))?;
            map.insert(name, Arc::new(RwLock::new(collection)));
        }

        Ok(TransientRelationshipMap::new(map))
    }
}

impl From<&TransientRelationshipMap> for TransientRelationshipMapWire {
    fn from(map: &TransientRelationshipMap) -> Self {
        let mut wire_map = BTreeMap::new();

        for (name, lock) in map.map.iter() {
            let collection = lock.read().expect("Failed to acquire read lock on holon collection");
            wire_map.insert(name.clone(), HolonCollectionWire::from(&*collection));
        }

        Self { map: wire_map }
    }
}
