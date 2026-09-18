use super::*;
use crate::core_shared_objects::{holon::SavedHolon, space_manager::HolonSpaceManager};
use crate::{HolonCollectionApi, ServiceRoutingPolicy, StagedReference, TransientReference};
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::{LocalId, PropertyMap, PropertyName};
use std::{any::Any, collections::HashMap};

#[derive(Debug)]
struct CountingService {
    scope: RelationshipCacheScope,
    edges: RwLock<HashMap<(u8, String), Vec<u8>>>,
    calls: RwLock<HashMap<(u8, String), usize>>,
    bulk_calls: AtomicUsize,
}

impl CountingService {
    fn new(scope: RelationshipCacheScope) -> Self {
        let mut edges = HashMap::new();
        for (source, name, targets) in [
            (1, "DescribedBy", vec![2]),
            (2, "InstanceRelationships", vec![5, 6, 12]),
            (2, "TargetOf", vec![9]),
            (5, "Extends", vec![3]),
            (6, "Extends", vec![3]),
            (7, "Extends", vec![4]),
            (9, "Extends", vec![3]),
            (12, "Extends", vec![3]),
            (8, "InstanceRelationships", vec![9]),
            (9, "SourceType", vec![8]),
            (9, "TargetType", vec![2]),
            (9, "HasInverse", vec![7]),
            (1, "Frozen", vec![10]),
            (1, "Moving", vec![10]),
            (1, "Incoming", vec![10]),
        ] {
            edges.insert((source, name.into()), targets);
        }
        Self {
            scope,
            edges: RwLock::new(edges),
            calls: RwLock::new(HashMap::new()),
            bulk_calls: AtomicUsize::new(0),
        }
    }
    fn count(&self, name: &str) -> usize {
        *self.calls.read().unwrap().get(&(1, name.into())).unwrap_or(&0)
    }
    fn reference(context: &Arc<TransactionContext>, id: u8) -> HolonReference {
        HolonReference::smart_with_key(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![id])),
            MapString(format!("node-{id}")),
        )
    }
}

impl HolonServiceApi for CountingService {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn relationship_cache_scope(&self) -> RelationshipCacheScope {
        self.scope
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        let number = id.local_id().0[0];
        let name = match number {
            3 => "DeclaredRelationshipType",
            4 => "InverseRelationshipType",
            5 => "Frozen",
            6 => "Moving",
            7 => "Incoming",
            9 => "Outgoing",
            12 => "DescribedBy",
            _ => "Type",
        };
        let mut properties = PropertyMap::new();
        properties.insert(PropertyName("TypeName".into()), BaseValue::StringValue(name.into()));
        properties.insert(
            PropertyName("Key".into()),
            BaseValue::StringValue(format!("node-{number}").into()),
        );
        properties.insert(
            PropertyName("IsDefinitional".into()),
            BaseValue::BooleanValue(MapBoolean(number != 6)),
        );
        Ok(Holon::Saved(SavedHolon::new(id.local_id().clone(), properties, None, MapInteger(1))))
    }
    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
        name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        let key = (id.local_id().0[0], name.to_string());
        *self.calls.write().unwrap().entry(key.clone()).or_default() += 1;
        let targets = self.edges.read().unwrap().get(&key).cloned().unwrap_or_default();
        // The cache must seal even a mutable collection supplied by the service.
        let mut collection = HolonCollection::new_transient();
        collection
            .add_references(targets.into_iter().map(|id| Self::reference(context, id)).collect())?;
        Ok(collection)
    }
    fn fetch_all_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        self.bulk_calls.fetch_add(1, Ordering::Relaxed);
        Err(HolonError::NotImplemented("bulk fetch must not be used".into()))
    }
    fn commit_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        panic!("unexpected commit")
    }
    fn delete_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &LocalId,
    ) -> Result<(), HolonError> {
        panic!("unexpected delete")
    }
    fn get_all_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        panic!("unexpected traversal")
    }
    fn load_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        panic!("unexpected load")
    }
}

#[test]
fn named_and_all_reads_share_definitional_cache_and_keep_mutable_membership_fresh(
) -> Result<(), HolonError> {
    for scope in
        [RelationshipCacheScope::RequestLocal, RelationshipCacheScope::SpaceDefinitionalOnly]
    {
        for named_first in [true, false] {
            let service = Arc::new(CountingService::new(scope));
            let space = Arc::new(HolonSpaceManager::new_with_managers(
                None,
                service.clone(),
                None,
                ServiceRoutingPolicy::BlockExternal,
            ));
            let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
            let source = CountingService::reference(&context, 1);
            let frozen_before =
                if named_first { Some(source.related_holons("Frozen")?) } else { None };
            let all = source.all_related_holons()?;
            let frozen =
                all.get_collection_for_relationship(&"Frozen".to_relationship_name()).unwrap();
            assert!(Arc::ptr_eq(&frozen, &source.related_holons("Frozen")?));
            if let Some(before) = frozen_before {
                assert!(Arc::ptr_eq(&before, &frozen));
            }
            assert_eq!(service.count("Frozen"), 1);
            assert_eq!(service.count("Moving"), 1);
            assert_eq!(service.count("Incoming"), 1);
            assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 0);
            for (_, collection) in all.iter() {
                assert!(
                    collection
                        .write()
                        .unwrap()
                        .add_references(vec![CountingService::reference(&context, 11)])
                        .is_err(),
                    "all reads must return sealed collections"
                );
            }
            for name in ["Moving", "Incoming"] {
                service.edges.write().unwrap().insert((1, name.into()), vec![11]);
            }
            let next = space.get_transaction_manager().open_public_transaction(space.clone())?;
            let source = CountingService::reference(&next, 1);
            let all = source.all_related_holons()?;
            assert!(Arc::ptr_eq(
                &frozen,
                &all.get_collection_for_relationship(&"Frozen".to_relationship_name()).unwrap()
            ));
            assert_eq!(service.count("Frozen"), 1);
            for name in ["Moving", "Incoming"] {
                let collection =
                    all.get_collection_for_relationship(&name.to_relationship_name()).unwrap();
                assert_eq!(
                    collection.read().unwrap().get_members(),
                    &vec![CountingService::reference(&next, 11)]
                );
                assert_eq!(service.count(name), 2);
                let named = source.related_holons(name)?;
                assert_eq!(
                    named.read().unwrap().get_members(),
                    collection.read().unwrap().get_members()
                );
                assert!(!Arc::ptr_eq(&named, &collection));
                assert_eq!(service.count(name), 3);
            }
            assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 0);
        }
    }
    Ok(())
}

#[test]
fn assert_cache_manager_is_thread_safe() {
    fn assert_thread_safe<T: Send + Sync>() {}
    assert_thread_safe::<HolonCacheManager>();
}

#[test]
fn cached_membership_bypasses_classification_even_during_resolution() -> Result<(), HolonError> {
    for scope in
        [RelationshipCacheScope::RequestLocal, RelationshipCacheScope::SpaceDefinitionalOnly]
    {
        for empty in [false, true] {
            let service = Arc::new(CountingService::new(scope));
            if empty {
                service.edges.write().unwrap().insert((1, "Frozen".into()), vec![]);
            }
            let space = Arc::new(HolonSpaceManager::new_with_managers(
                None,
                service.clone(),
                None,
                ServiceRoutingPolicy::BlockExternal,
            ));
            let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
            let manager = HolonCacheManager::new(service.clone());
            let id = HolonId::Local(LocalId(vec![1]));
            let name = "Frozen".to_relationship_name();
            let first = manager.get_related_holons(&context, &id, &name)?;
            let before = service.calls.read().unwrap().clone();
            let _guard = manager.enter_relationship_semantics_resolution();
            let again = manager.get_related_holons(&context, &id, &name)?;
            assert!(Arc::ptr_eq(&first, &again));
            let cache = manager.relationship_cache.read().unwrap().clone();
            let hit = cache.related_holons(&context, service.as_ref(), &id, &name, || {
                panic!("cache hits must not resolve descriptors")
            })?;
            assert!(Arc::ptr_eq(&first, &hit));
            assert_eq!(*service.calls.read().unwrap(), before);
            let moving =
                manager.get_related_holons(&context, &id, &"Moving".to_relationship_name())?;
            let again =
                manager.get_related_holons(&context, &id, &"Moving".to_relationship_name())?;
            assert!(!Arc::ptr_eq(&moving, &again));
        }
    }
    Ok(())
}

#[test]
fn miss_fetches_before_classification_and_failed_classification_is_not_cached(
) -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(RelationshipCacheScope::RequestLocal));
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = RelationshipCache::new();
    let id = HolonId::Local(LocalId(vec![1]));
    let name = "Frozen".to_relationship_name();
    let result = cache.related_holons(&context, service.as_ref(), &id, &name, || {
        assert_eq!(service.count("Frozen"), 1);
        Err(HolonError::InvalidState("classification failed".into()))
    });
    assert!(matches!(result, Err(HolonError::InvalidState(_))));
    cache.related_holons(&context, service.as_ref(), &id, &name, || {
        assert_eq!(service.count("Frozen"), 2);
        Ok(RelationshipCachePolicy::Reuse)
    })?;
    Ok(())
}
