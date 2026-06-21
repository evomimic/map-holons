use super::{
    holon_pool::{HolonPool, StagedHolonPool},
    nursery_access_internal::NurseryAccessInternal,
    Holon,
};
use crate::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use crate::{
    core_shared_objects::{transient_holon_manager::ToHolonCloneModel, StagedHolon},
    reference_layer::{HolonStagingBehavior, StagedReference, TransientReference},
    HolonReference, NurseryAccess, ReadableHolon, SmartReference, WritableHolon,
};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, HolonId, TemporaryId};
use std::{
    any::Any,
    sync::{Arc, RwLock, Weak},
};
use type_names::CorePropertyTypeName;

#[derive(Clone, Debug)]
pub struct Nursery {
    tx_id: TxId,
    context: Weak<TransactionContext>,
    // `StagedHolonPool` is a typed wrapper over the generic `HolonPool`; entries are
    // intentionally stored as `Holon::Staged` to preserve a single pool abstraction.
    staged_holons: Arc<RwLock<StagedHolonPool>>,
}

// The Nursery uses `Arc<RwLock<StagedHolonPool>>` to allow thread-safe mutation of staged holons.
// Each holon is stored in a `HolonPool`, where Holons are individually wrapped in `Arc<RwLock<Holon>>`.
// This structure allows:
//
// - Shared concurrent reads across threads (e.g. for base key lookups)
// - Exclusive mutation when staging or clearing
// - Safe export/import of holons via a runtime `HolonPool`
// - Isolation of each staged holon for commit preparation, minimizing lock contention.
//
// All read/write access to the staged pool is explicitly scoped to avoid lock poisoning or panic scenarios.
impl Nursery {
    // /// Creates a new Nursery with an empty HolonPool
    pub fn new(tx_id: TxId, context: Weak<TransactionContext>) -> Self {
        Self {
            tx_id,
            context,
            staged_holons: Arc::new(RwLock::new(StagedHolonPool(HolonPool::new()))),
        }
    }

    /// Stages a new holon.
    ///
    /// # Arguments
    /// * `holon` - A reference to the holon to be staged.
    ///
    /// # Returns
    /// The TemporaryId, which is used a unique identifier.
    fn stage_holon(&self, holon: StagedHolon) -> Result<TemporaryId, HolonError> {
        let mut pool = self.staged_holons.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged_holons: {}",
                e
            ))
        })?;
        let id = pool.insert_holon(Holon::Staged(holon))?;
        Ok(id)
    }

    /// This function converts a TemporaryId into a StagedReference.
    /// Returns HolonError::HolonNotFound if id is not present in the holon pool.
    fn to_validated_staged_reference(
        &self,
        id: &TemporaryId,
    ) -> Result<StagedReference, HolonError> {
        // Validate that this id exists in the nursery's staged pool.
        let _ = self.get_holon_by_id(id)?;

        let transaction_handle = self.require_handle()?;
        Ok(StagedReference::from_temporary_id(transaction_handle, id))
    }

    fn require_handle(&self) -> Result<TransactionContextHandle, HolonError> {
        let context = self.context.upgrade().ok_or_else(|| {
            HolonError::ServiceNotAvailable(format!(
                "TransactionContext (tx_id={})",
                self.tx_id.value()
            ))
        })?;

        debug_assert_eq!(
            context.tx_id(),
            self.tx_id,
            "Nursery context tx_id mismatch: context={} nursery={}",
            context.tx_id().value(),
            self.tx_id.value()
        );

        // Extra runtime guard for non-debug builds.
        if context.tx_id() != self.tx_id {
            return Err(HolonError::CrossTransactionReference {
                reference_kind: "Nursery".to_string(),
                reference_id: format!("TxId={}", self.tx_id.value()),
                reference_tx: self.tx_id.value(),
                context_tx: context.tx_id().value(),
            });
        }

        Ok(TransactionContextHandle::new(context))
    }
}

impl NurseryAccess for Nursery {
    /// Retrieves a staged holon by index.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Arc<RwLock<Holon>>, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        pool.get_holon_by_id(id)
    }
}

impl HolonStagingBehavior for Nursery {
    // Caller is assuming there is only one, returns duplicate error if multiple.
    fn get_staged_holon_by_base_key(&self, key: &MapString) -> Result<StagedReference, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        let id = pool.get_id_by_base_key(key)?;
        self.to_validated_staged_reference(&id)
    }

    fn get_staged_holons_by_base_key(
        &self,
        key: &MapString,
    ) -> Result<Vec<StagedReference>, HolonError> {
        let mut staged_references = Vec::new();
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        let ids = pool.get_ids_by_base_key(key)?;
        for id in ids {
            let validated_staged_reference = self.to_validated_staged_reference(&id)?;
            staged_references.push(validated_staged_reference);
        }

        Ok(staged_references)
    }

    /// Does a lookup by full (unique) key on staged holons.
    fn get_staged_holon_by_versioned_key(
        &self,
        key: &MapString,
    ) -> Result<StagedReference, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        let id = pool.get_id_by_versioned_key(key)?;
        self.to_validated_staged_reference(&id)
    }

    fn staged_count(&self) -> Result<i64, HolonError> {
        let count = self
            .staged_holons
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged_holons: {}",
                    e
                ))
            })?
            .len() as i64;
        Ok(count)
    }

    fn stage_new_holon(
        &self,
        transient_reference: TransientReference,
    ) -> Result<StagedReference, HolonError> {
        let staged_holon =
            StagedHolon::new_from_clone_model(transient_reference.holon_clone_model()?)?;
        let new_id = self.stage_holon(staged_holon)?;
        self.to_validated_staged_reference(&new_id)
    }

    /// Stage a new holon by cloning an existing holon, with a new key and
    /// *without* maintaining lineage to the original.
    fn stage_new_from_clone(
        &self,
        original_holon: HolonReference,
        new_key: MapString,
    ) -> Result<StagedReference, HolonError> {
        // Clone into a transient holon
        let mut cloned_transient = original_holon.clone_holon()?;

        // Overwrite the Key property on the clone
        let key_prop = CorePropertyTypeName::Key.as_property_name();
        cloned_transient.with_property_value(key_prop, BaseValue::StringValue(new_key))?;

        // Reset original_id (this is a new clone, not a new version)
        cloned_transient.reset_original_id()?;

        // Stage the cloned holon
        let mut cloned_staged = self.stage_new_holon(cloned_transient)?;

        // Explicitly clear predecessor for "from clone" semantics
        cloned_staged.with_predecessor(None)?;

        Ok(cloned_staged)
    }

    /// Stage an existing holon for possible version-producing or graph-only mutation.
    fn stage_new_version(
        &self,
        current_version: SmartReference,
    ) -> Result<StagedReference, HolonError> {
        let source_local_id = match current_version.holon_id() {
            HolonId::Local(local_id) => local_id,
            HolonId::External(_) => {
                return Err(HolonError::InvalidParameter(
                    "stage_new_version requires a local persisted holon".to_string(),
                ))
            }
        };

        // Clone through the reference layer so cached persisted relationships are preserved.
        let cloned_transient = current_version.clone_holon()?;
        let staged_holon = StagedHolon::new_for_update_from_clone_model(
            cloned_transient.holon_clone_model()?,
            source_local_id,
        )?;

        let new_id = self.stage_holon(staged_holon)?;
        let mut staged_reference = self.to_validated_staged_reference(&new_id)?;

        // A new version's lineage is established at commit time. Drop any
        // predecessor edge cloned from the source version so old lineage cannot
        // be replayed onto the staged successor.
        staged_reference.with_predecessor(None)?;

        Ok(staged_reference)
    }
}

impl NurseryAccessInternal for Nursery {
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// Clears all staged holons.
    fn clear_stage(&self) -> Result<(), HolonError> {
        // Failure to acquire the lock is propagated as an error rather than panicking.
        let mut guard = self.staged_holons.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged_holons: {}",
                e
            ))
        })?;
        guard.clear();
        Ok(())
    }

    fn get_id_by_versioned_key(&self, key: &MapString) -> Result<TemporaryId, HolonError> {
        let pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        pool.get_id_by_versioned_key(key)
    }

    /// Exports the staged holons as a runtime `HolonPool`.
    fn export_staged_holons(&self) -> Result<HolonPool, HolonError> {
        let staged_pool = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        Ok(staged_pool.0.clone())
    }

    /// Replaces the current staged holons with those from the provided `HolonPool`.
    fn import_staged_holons(&self, pool: HolonPool) -> Result<(), HolonError> {
        let mut guard = self.staged_holons.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged_holons: {}",
                e
            ))
        })?;
        guard.import_pool(pool); // Mutates the existing pool
        Ok(())
    }

    /// Returns a reference-layer view of all staged holons as `StagedReference`s.
    ///
    /// This is the main entry for the commit pipeline and avoids exposing
    /// the underlying HolonPool or `Arc<RwLock<Holon>>` handles.
    fn get_staged_references(&self) -> Result<Vec<StagedReference>, HolonError> {
        let transaction_handle = self.require_handle()?;
        let guard = self.staged_holons.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged_holons: {}",
                e
            ))
        })?;
        Ok(guard.get_staged_references(transaction_handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core_shared_objects::{
            holon::{SavedHolon, StagedState},
            holon_behavior::ReadableHolonState,
            space_manager::HolonSpaceManager,
            RelationshipMap, ServiceRoutingPolicy,
        },
        reference_layer::{HolonServiceApi, ReadableHolon},
        HolonCollection, HolonCollectionApi,
    };
    use base_types::{BaseValue, MapInteger, MapString};
    use core_types::{LocalId, PropertyMap, PropertyName, RelationshipName};
    use std::any::Any;
    use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, ToRelationshipName};

    #[derive(Debug)]
    struct StageVersionTestService {
        source_id: LocalId,
        source_holon: SavedHolon,
        source_predecessor_id: Option<LocalId>,
    }

    impl HolonServiceApi for StageVersionTestService {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn commit_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _staged_references: &[StagedReference],
        ) -> Result<TransientReference, HolonError> {
            Err(HolonError::NotImplemented("commit_internal".to_string()))
        }

        fn delete_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _local_id: &LocalId,
        ) -> Result<(), HolonError> {
            Err(HolonError::NotImplemented("delete_holon_internal".to_string()))
        }

        fn fetch_all_related_holons_internal(
            &self,
            context: &Arc<TransactionContext>,
            source_id: &HolonId,
        ) -> Result<RelationshipMap, HolonError> {
            if source_id.local_id() == &self.source_id {
                let mut relationships = RelationshipMap::new_empty();

                if let Some(predecessor_id) = &self.source_predecessor_id {
                    let predecessor_reference = HolonReference::smart_with_key(
                        context.context_handle(),
                        HolonId::Local(predecessor_id.clone()),
                        MapString("prior-version".to_string()),
                    );
                    let mut predecessor_collection = HolonCollection::new_existing();
                    predecessor_collection.add_references(vec![predecessor_reference])?;
                    relationships.insert(
                        CoreRelationshipTypeName::Predecessor.to_relationship_name(),
                        Arc::new(RwLock::new(predecessor_collection)),
                    );
                }

                Ok(relationships)
            } else {
                Err(HolonError::HolonNotFound(format!("{:?}", source_id)))
            }
        }

        fn fetch_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            id: &HolonId,
        ) -> Result<Holon, HolonError> {
            if id.local_id() == &self.source_id {
                Ok(Holon::Saved(self.source_holon.clone()))
            } else {
                Err(HolonError::HolonNotFound(format!("{:?}", id)))
            }
        }

        fn fetch_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
            _relationship_name: &RelationshipName,
        ) -> Result<HolonCollection, HolonError> {
            Ok(HolonCollection::new_existing())
        }

        fn get_all_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
        ) -> Result<HolonCollection, HolonError> {
            Err(HolonError::NotImplemented("get_all_holons_internal".to_string()))
        }

        fn load_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _load_set: TransientReference,
        ) -> Result<TransientReference, HolonError> {
            Err(HolonError::NotImplemented("load_holons_internal".to_string()))
        }
    }

    fn stage_version_test_context(
        source_id: LocalId,
        source_holon: SavedHolon,
        source_predecessor_id: Option<LocalId>,
    ) -> Arc<TransactionContext> {
        let holon_service: Arc<dyn HolonServiceApi> =
            Arc::new(StageVersionTestService { source_id, source_holon, source_predecessor_id });
        let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            holon_service,
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));

        space_manager
            .get_transaction_manager()
            .open_new_transaction(Arc::clone(&space_manager))
            .expect("test transaction should open")
    }

    #[test]
    fn stage_new_version_enters_update_lifecycle_without_predecessor_edge() -> Result<(), HolonError>
    {
        let source_id = LocalId(vec![1, 2, 3]);
        let original_title = BaseValue::StringValue(MapString("Original Title".to_string()));
        let mut properties = PropertyMap::new();
        let title = PropertyName(MapString("Title".to_string()));
        properties.insert(
            CorePropertyTypeName::Key.as_property_name(),
            BaseValue::StringValue(MapString("book-one".to_string())),
        );
        properties.insert(title.clone(), original_title.clone());
        let source_holon = SavedHolon::new(source_id.clone(), properties, None, MapInteger(3));
        let context = stage_version_test_context(
            source_id.clone(),
            source_holon,
            Some(LocalId(vec![9, 8, 7])),
        );
        let current_version = SmartReference::new_from_id(
            context.context_handle(),
            HolonId::Local(source_id.clone()),
        );

        let staged_reference = context.mutation().stage_new_version(current_version)?;

        assert!(staged_reference.is_in_state(&context, StagedState::ForUpdate)?);
        assert!(staged_reference.predecessor()?.is_none());

        let staged_holon = staged_reference.get_holon_to_commit(&context)?;
        let staged_holon = staged_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;
        let Holon::Staged(staged_holon) = &*staged_holon else {
            panic!("stage_new_version should stage a StagedHolon");
        };

        assert_eq!(staged_holon.versioned_source_id_ref(), Some(&source_id));
        assert_eq!(staged_holon.original_id_ref(), None);
        assert_eq!(staged_holon.version(), &MapInteger(3));
        assert_eq!(staged_holon.property_value(&title)?, Some(original_title));

        Ok(())
    }
}
