use super::TransactionContext;
use crate::{HolonCollection, StagedReference, TransientReference};
use base_types::MapString;
use core_types::HolonError;
use std::sync::Arc;

/// Semantic facade for transaction-scoped lookup operations.
#[derive(Debug, Clone)]
pub struct LookupFacade {
    pub(crate) context: Arc<TransactionContext>,
}

impl LookupFacade {
    pub fn get_all_holons(&self) -> Result<HolonCollection, HolonError> {
        self.context.get_holon_service().get_all_holons_internal(&self.context)
    }

    /// Convenience method for retrieving a single StagedReference for a base key, when the caller expects there to only be one.
    /// Returns a duplicate error if multiple found.
    pub fn get_staged_holon_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<StagedReference, HolonError> {
        self.context.get_staging_service().get_staged_holon_by_base_key(key)
    }

    /// Returns StagedReference's for all Holons that have the same base key.
    /// This can be useful if multiple versions of the same Holon are being staged at the same time.
    pub fn get_staged_holons_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<Vec<StagedReference>, HolonError> {
        self.context.get_staging_service().get_staged_holons_by_base_key(key)
    }

    /// Does a lookup by full (unique) key on staged holons.
    pub fn get_staged_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<StagedReference, HolonError> {
        self.context.get_staging_service().get_staged_holon_by_versioned_key(key)
    }

    /// Convenience method for retrieving a single TransientReference for a base key, when the caller expects there to only be one.
    /// Returns a duplicate error if multiple found.
    pub fn get_transient_holon_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError> {
        self.context.get_transient_behavior_service().get_transient_holon_by_base_key(key)
    }

    /// Does a lookup by full (unique) key on transient holons.
    pub fn get_transient_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<TransientReference, HolonError> {
        self.context.get_transient_behavior_service().get_transient_holon_by_versioned_key(key)
    }

    // Helpers

    // Gets total count of Staged Holons present in the Nursery
    pub fn staged_count(&self) -> Result<i64, HolonError> {
        self.context.get_staging_service().staged_count()
    }

    // Gets total count of Transient Holons present in the TransientHolonManager
    pub fn transient_count(&self) -> Result<i64, HolonError> {
        self.context.get_transient_behavior_service().transient_count()
    }
}
