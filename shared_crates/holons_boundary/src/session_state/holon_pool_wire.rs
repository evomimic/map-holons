use crate::context_binding::HolonWire;
use base_types::MapString;
use core_types::{HolonError, TemporaryId};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::{Holon, HolonPool};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SerializableHolonPool {
    pub holons: BTreeMap<TemporaryId, HolonWire>,
    pub keyed_index: BTreeMap<MapString, TemporaryId>,
}

impl Default for SerializableHolonPool {
    fn default() -> Self {
        Self { holons: BTreeMap::new(), keyed_index: BTreeMap::new() }
    }
}

impl SerializableHolonPool {
    /// Binds a wire pool to the supplied transaction, producing a runtime HolonPool.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<HolonPool, HolonError> {
        let mut holons: BTreeMap<TemporaryId, Arc<RwLock<Holon>>> = BTreeMap::new();

        for (id, holon_wire) in self.holons {
            let holon_runtime = holon_wire.bind(context)?;
            holons.insert(id, Arc::new(RwLock::new(holon_runtime)));
        }

        Ok(HolonPool::from_parts(holons, self.keyed_index))
    }
}

impl From<&HolonPool> for SerializableHolonPool {
    fn from(pool: &HolonPool) -> Self {
        let mut holons = BTreeMap::new();
        for (id, holon) in pool.holons_by_id() {
            holons.insert(
                id.clone(),
                HolonWire::from(&*holon.read().expect("Failed to acquire read lock on holon")),
            );
        }

        Self { holons, keyed_index: pool.keyed_index().clone() }
    }
}

impl From<HolonPool> for SerializableHolonPool {
    fn from(pool: HolonPool) -> Self {
        SerializableHolonPool::from(&pool)
    }
}
