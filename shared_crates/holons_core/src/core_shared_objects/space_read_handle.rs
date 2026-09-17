//! Opaque, space-scoped capability for resolving saved references.

use std::sync::{Arc, RwLock};

use core_types::{HolonError, HolonId, RelationshipName};

use crate::core_shared_objects::{Holon, HolonCollection};
use crate::reference_layer::HolonSpaceBehavior;
use crate::RelationshipMap;

use super::space_manager::HolonSpaceManager;

/// A read capability bound to a holon space rather than a caller transaction.
///
/// Saved references retain this handle.  A cache miss is executed using a fresh,
/// restricted transaction owned by the space; that transaction is deliberately
/// not exposed through the reference API.
#[derive(Clone, Debug)]
pub struct SpaceReadHandle {
    space_manager: Arc<HolonSpaceManager>,
}

impl SpaceReadHandle {
    pub(crate) fn new(space_manager: Arc<HolonSpaceManager>) -> Self {
        Self { space_manager }
    }

    pub(crate) fn get_rc_holon(
        &self,
        holon_id: &HolonId,
    ) -> Result<Arc<RwLock<Holon>>, HolonError> {
        let cache = self.space_manager.get_cache_access();
        if let Some(cached) = cache.get_cached_rc_holon(holon_id)? {
            return Ok(cached);
        }
        self.with_cache_context(|context| {
            context
                .cache_access(crate::reference_layer::smart_reference::SmartRefAccessKey::new())
                .get_rc_holon(&context, holon_id)
        })
    }

    pub(crate) fn get_related_holons(
        &self,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.with_cache_context(|context| {
            context
                .cache_access(crate::reference_layer::smart_reference::SmartRefAccessKey::new())
                .get_related_holons(&context, source_holon_id, relationship_name)
        })
    }

    pub(crate) fn get_all_related_holons(
        &self,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        self.with_cache_context(|context| {
            context
                .cache_access(crate::reference_layer::smart_reference::SmartRefAccessKey::new())
                .get_all_related_holons(&context, source_holon_id)
        })
    }

    pub(crate) fn same_space(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.space_manager, &other.space_manager)
    }

    pub(crate) fn open_resolution_context(
        &self,
    ) -> Result<Arc<super::transactions::TransactionContext>, HolonError> {
        self.space_manager
            .get_transaction_manager()
            .open_restricted_cache_read_transaction(Arc::clone(&self.space_manager))
    }

    fn with_cache_context<T>(
        &self,
        operation: impl FnOnce(Arc<super::transactions::TransactionContext>) -> Result<T, HolonError>,
    ) -> Result<T, HolonError> {
        let context = self
            .space_manager
            .get_transaction_manager()
            .open_restricted_cache_read_transaction(Arc::clone(&self.space_manager))?;
        operation(context)
    }
}
