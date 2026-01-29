use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

use super::{Holon, HolonWire, ReadableHolonState, WriteableHolonState};
use crate::core_shared_objects::transactions::{TransactionContext, TransactionContextHandle};
use crate::utils::uuid::create_temporary_id_from_key;
use crate::StagedReference;
use base_types::MapString;
use core_types::{HolonError, TemporaryId};
//
// === HolonPool NewTypes ===
//

#[derive(Debug, Clone)]
pub struct TransientHolonPool(pub HolonPool);

impl From<HolonPool> for TransientHolonPool {
    fn from(pool: HolonPool) -> Self {
        TransientHolonPool(pool)
    }
}

impl Deref for TransientHolonPool {
    type Target = HolonPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TransientHolonPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone)]
pub struct StagedHolonPool(pub HolonPool);

impl From<HolonPool> for StagedHolonPool {
    fn from(pool: HolonPool) -> Self {
        StagedHolonPool(pool)
    }
}

impl Deref for StagedHolonPool {
    type Target = HolonPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StagedHolonPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransientSerializableHolonPool(pub SerializableHolonPool);

impl Deref for TransientSerializableHolonPool {
    type Target = SerializableHolonPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TransientSerializableHolonPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StagedSerializableHolonPool(pub SerializableHolonPool);

impl Deref for StagedSerializableHolonPool {
    type Target = SerializableHolonPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StagedSerializableHolonPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

//
// === SerializableHolonPool ===
//

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
    pub fn bind(self, context: Arc<TransactionContext>) -> Result<HolonPool, HolonError> {
        let mut holons = BTreeMap::new();

        for (id, holon_wire) in self.holons {
            let holon_runtime = holon_wire.bind(Arc::clone(&context))?;
            holons.insert(id, Arc::new(RwLock::new(holon_runtime)));
        }

        Ok(HolonPool { holons, keyed_index: self.keyed_index })
    }
}

//
// === HolonPool ===
//
// HolonPool no longer derives `PartialEq` or `Eq` because it stores Holons as `Arc<RwLock<Holon>>`.
// These types do not implement equality by default, and comparing them would require
// acquiring locks and comparing underlying Holon values, which is non-trivial and potentially blocking.
//
// Instead, equality comparisons should be done on `SerializableHolonPool`, which is derived from HolonPool
// and contains plain, serializable Holons. It continues to derive `PartialEq` and `Eq` for testing and export validation.

/// A general-purpose container that manages owned Holons with key-based and index-based lookups.
#[derive(Debug, Clone)]
pub struct HolonPool {
    holons: BTreeMap<TemporaryId, Arc<RwLock<Holon>>>,
    keyed_index: BTreeMap<MapString, TemporaryId>,
}

impl HolonPool {
    /// Creates an empty HolonPool
    pub fn new() -> Self {
        Self { holons: BTreeMap::new(), keyed_index: BTreeMap::new() }
    }

    /// Clears all Holons and their associated key mappings.
    pub fn clear(&mut self) {
        self.holons.clear();
        self.keyed_index.clear();
    }

    /// Get a vector of references to the Holons in the HolonPool.
    ///
    /// ⚠️ Only intended during the commit process due to mutable access risks.
    pub fn get_all_holons(&self) -> Vec<Arc<RwLock<Holon>>> {
        self.holons.values().cloned().collect()
    }

    /// Retrieves a Holon by its temporary id.
    pub fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Arc<RwLock<Holon>>, HolonError> {
        self.holons
            .get(id)
            .cloned()
            .ok_or_else(|| HolonError::HolonNotFound(format!("for id: {:?}", id)))
    }

    /// Retrieves a Holon by its versioned (unique) key.
    pub fn get_holon_by_versioned_key(&self, key: &MapString) -> Option<Arc<RwLock<Holon>>> {
        self.keyed_index.get(key).and_then(|id| self.holons.get(id).cloned())
    }

    /// Retrieves the temporary id of a Holon by its **base key**.
    ///
    /// Assumptions:
    /// - Versioned keys are stored in `keyed_index` using the convention
    ///   `"<base_key>__<version>_transient"`.
    /// - This helper is used when the caller expects **exactly one**
    ///   Holon for the given base key (across all versions).
    ///
    /// Behavior:
    /// - If no entries match the base key → `HolonNotFound`.
    /// - If more than one entry matches (multiple versions) → `DuplicateError`.
    /// - Otherwise returns the single matching `TemporaryId`.
    pub fn get_id_by_base_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        let ids = self.get_ids_by_base_key(key)?;

        if ids.len() > 1 {
            return Err(HolonError::DuplicateError("Holons".to_string(), format!("key: {}", key)));
        }

        // Safe: we already know len() > 0 from get_ids_by_base_key
        Ok(ids[0].clone())
    }

    /// Returns `TemporaryId`s for all Holons that share the same **base key**.
    ///
    /// A "base key" is the logical key stored in the Holon's `Key` property.
    /// Versioned keys in the pool are expected to follow the convention:
    ///
    /// - `"<base_key>__<version>_transient"`
    ///
    /// This function:
    /// - Includes an exact match for `base_key` if present.
    /// - Includes all versioned keys whose string starts with `"<base_key>__"`.
    ///
    /// Examples:
    /// - base `"TypeKind"` matches:
    ///     - `"TypeKind"`
    ///     - `"TypeKind__7_transient"`, `"TypeKind__8_transient"`, ...
    /// - base `"TypeKind.Property"` matches:
    ///     - `"TypeKind.Property__12_transient"`, ...
    /// - base `"TypeKind"` does **not** match `"TypeKind.Property__..."`.
    pub fn get_ids_by_base_key(&self, key: &MapString) -> Result<Vec<&TemporaryId>, HolonError> {
        // Prefix that delimits the version section of the key.
        // This prevents collisions like "TypeKind" vs "TypeKind.Property".
        let versioned_prefix = format!("{}__", key.0);
        let start = MapString(versioned_prefix.clone());

        let mut ids: Vec<&TemporaryId> = Vec::new();

        // 1) Include an exact base-key match if one exists (defensive; not all pools
        //    are guaranteed to only store versioned keys).
        if let Some((_, id)) = self.keyed_index.get_key_value(key) {
            ids.push(id);
        }

        // 2) Collect all versioned entries for this base key:
        //    keys in the form "<base_key>__<version>_transient".
        //
        // Because `keyed_index` is ordered, all such keys form a contiguous range
        // starting at `versioned_prefix`. We walk until the prefix no longer matches.
        for (k, v) in self.keyed_index.range(start..) {
            if !k.0.starts_with(&versioned_prefix) {
                break;
            }
            ids.push(v);
        }

        if ids.is_empty() {
            return Err(HolonError::HolonNotFound(format!("for key: {}", key)));
        }

        Ok(ids)
    }

    /// Retrieves the temporary id of a Holon by its versioned key.
    pub fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        self.keyed_index
            .get(key)
            .cloned()
            .ok_or_else(|| HolonError::HolonNotFound(format!("for key: {}", key)))
    }

    /// Returns a vector of `StagedReference`s for all holons currently staged in this pool.
    ///
    /// This provides a reference-layer view of the pool contents without exposing
    /// the underlying Holon structs or locks. All returned references are explicitly
    /// bound to the supplied transaction context.
    pub fn get_staged_references(
        &self,
        transaction_handle: TransactionContextHandle,
    ) -> Vec<StagedReference> {
        self.holons
            .keys()
            .map(|temp_id| StagedReference::from_temporary_id(transaction_handle.clone(), temp_id))
            .collect()
    }

    /// Exports the HolonPool as a `SerializableHolonPool`.
    pub fn export_pool(&self) -> Result<SerializableHolonPool, HolonError> {
        let mut holons = BTreeMap::new();
        for (id, holon) in self.holons.iter() {
            // Read lock the holon to clone its value
            holons.insert(
                id.clone(),
                HolonWire::from(&*holon.read().expect("Failed to acquire read lock on holon")),
            );
        }
        Ok(SerializableHolonPool { holons, keyed_index: self.keyed_index.clone() })
    }

    /// Imports a `SerializableHolonPool`, replacing the current holons.
    pub fn import_pool(&mut self, pool: HolonPool) {
        self.holons.clear();
        self.keyed_index.clear();
        self.holons.extend(pool.holons);
        self.keyed_index.extend(pool.keyed_index);
    }

    /// Inserts a new Holon into the pool.
    pub fn insert_holon(&mut self, mut holon: Holon) -> Result<TemporaryId, HolonError> {
        let mut versioned_key = holon.versioned_key()?;

        while self.keyed_index.get(&versioned_key).is_some() {
            holon.increment_version()?;
            versioned_key = holon.versioned_key()?;
        }

        let id = create_temporary_id_from_key(&versioned_key);

        self.keyed_index.insert(versioned_key, id.clone());
        // Store new holon wrapped in Arc<RwLock>
        self.holons.insert(id.clone(), Arc::new(RwLock::new(holon)));

        Ok(id)
    }

    /// Returns the number of Holons in the pool.
    pub fn len(&self) -> usize {
        self.holons.len()
    }
}
