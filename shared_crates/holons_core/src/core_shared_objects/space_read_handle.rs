//! Opaque, space-scoped capability for resolving saved references.

use std::sync::{Arc, RwLock};

use core_types::{HolonError, HolonId, RelationshipName};

use crate::core_shared_objects::{Holon, HolonCollection};
use crate::reference_layer::HolonSpaceBehavior;
use crate::RelationshipMap;

use super::space_manager::HolonSpaceManager;

/// A read capability bound to a holon space rather than a caller transaction.
///
/// Saved references retain this handle. Cache managers own the private execution
/// contexts needed to fill saved-state caches.
#[derive(Clone, Debug)]
pub struct SpaceReadHandle {
    space_manager: Arc<HolonSpaceManager>,
}

impl SpaceReadHandle {
    /// Populates explicit saved identities through a restricted cache-read context.
    /// Success is implicit and ordinarily evictable; no retention leases are returned.
    pub fn ensure_cached(
        &self,
        ids: &[HolonId],
    ) -> Vec<super::cache_population::CachePopulationError> {
        if ids.is_empty() {
            return Vec::new();
        }
        self.space_manager.get_cache_access().ensure_cached(ids)
    }

    pub(crate) fn new(space_manager: Arc<HolonSpaceManager>) -> Self {
        Self { space_manager }
    }

    pub(crate) fn get_rc_holon(
        &self,
        holon_id: &HolonId,
    ) -> Result<Arc<RwLock<Holon>>, HolonError> {
        self.space_manager.get_cache_access().get_rc_holon(holon_id)
    }

    pub(crate) fn get_related_holons(
        &self,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.space_manager.get_cache_access().get_related_holons(source, name)
    }

    pub(crate) fn get_related_holons_with_hint(
        &self,
        source: &HolonId,
        name: &RelationshipName,
        hint: crate::RelationshipReadHint,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.space_manager.get_cache_access().get_related_holons_with_hint(source, name, hint)
    }

    pub(crate) fn retain_holon(
        &self,
        id: &HolonId,
    ) -> Result<super::holon_cache::SavedHolonRetention, HolonError> {
        self.space_manager.get_cache_access().retain_holon(id)
    }

    pub(crate) fn prepare_relationships(
        &self,
        source: &HolonId,
        names: &[RelationshipName],
    ) -> Result<(), HolonError> {
        self.space_manager.get_cache_access().prepare_relationships(source, names)
    }

    pub(crate) fn get_all_related_holons(
        &self,
        source: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        self.space_manager.get_cache_access().get_all_related_holons(source)
    }

    pub(crate) fn same_space(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.space_manager, &other.space_manager)
    }

    /// Reads the associated Session registry without allocating an execution context.
    pub(crate) fn descriptor_registry(
        &self,
    ) -> Option<Arc<super::descriptor_registry::EffectiveHolonDescriptorRegistry>> {
        self.space_manager.descriptor_registry()
    }

    /// Resolves a saved semantic key through the existing service/Dance path.
    /// Only the returned Space-bound reference escapes; temporary transport state is disposed.
    pub(crate) fn get_saved_holon_by_key(
        &self,
        key: &base_types::MapString,
    ) -> Result<crate::SmartReference, HolonError> {
        let context = self
            .space_manager
            .get_transaction_manager()
            .open_private_saved_read(self.space_manager.clone())?;
        self.space_manager.get_holon_service().get_saved_holon_by_key_internal(&context, key)
    }

    /// Allocates an owner only for descriptor construction without a Session registry.
    /// The returned transient graph retains this owner; it cannot be disposed at call exit.
    /// Callers with an existing owner must use that owner instead.
    pub(crate) fn open_descriptor_construction_context(
        &self,
    ) -> Result<Arc<super::transactions::TransactionContext>, HolonError> {
        self.space_manager
            .get_transaction_manager()
            .open_private_restricted_cache_read_transaction(Arc::clone(&self.space_manager))
    }
}
