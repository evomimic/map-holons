use super::*;
use crate::core_shared_objects::{holon::SavedHolon, space_manager::HolonSpaceManager};
use crate::RelationshipCachePolicy;
use crate::{HolonCollectionApi, ServiceRoutingPolicy, StagedReference, TransientReference};
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::{LocalId, PropertyMap, PropertyName};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::{any::Any, collections::HashMap};
use type_names::ToRelationshipName;

#[derive(Debug)]
struct CountingService {
    reuse_frozen: bool,
    edges: RwLock<HashMap<(u8, String), Vec<u8>>>,
    calls: RwLock<HashMap<(u8, String), usize>>,
    bulk_calls: AtomicUsize,
    clock: AtomicU64,
}

impl CountingService {
    fn new(reuse_frozen: bool) -> Self {
        let mut edges = HashMap::new();
        for (source, name, targets) in [
            (1, "DescribedBy", vec![2]),
            (2, "InstanceRelationships", vec![5, 6, 12]),
            (2, "SourceOf", vec![7]),
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
            reuse_frozen,
            edges: RwLock::new(edges),
            calls: RwLock::new(HashMap::new()),
            bulk_calls: AtomicUsize::new(0),
            clock: AtomicU64::new(0),
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
    fn relationship_cache_time_millis(&self) -> Option<u64> {
        Some(self.clock.load(Ordering::Relaxed))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn relationship_cache_policy(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        name: &RelationshipName,
    ) -> Result<RelationshipCachePolicy, HolonError> {
        Ok(if self.reuse_frozen && name.to_string() == "Frozen" {
            RelationshipCachePolicy::Reuse
        } else {
            RelationshipCachePolicy::Fresh
        })
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
    {
        for named_first in [true, false] {
            let service = Arc::new(CountingService::new(true));
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
fn cached_membership_bypasses_classification() -> Result<(), HolonError> {
    {
        for empty in [false, true] {
            let service = Arc::new(CountingService::new(true));
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
    let service = Arc::new(CountingService::new(false));
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

#[derive(Debug)]
struct GuestLikeService(CountingService);
impl HolonServiceApi for GuestLikeService {
    fn relationship_cache_policy(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        _: &RelationshipName,
    ) -> Result<RelationshipCachePolicy, HolonError> {
        Ok(RelationshipCachePolicy::Reuse)
    }

    fn fetch_all_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        panic!("unexpected bulk fetch")
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
        name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        self.0.fetch_related_holons_internal(context, id, name)
    }
    fn fetch_holon_internal(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        self.0.fetch_holon_internal(context, id)
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
fn guest_kernel_policy_bootstraps_without_descriptor_traversal() -> Result<(), HolonError> {
    let service = Arc::new(GuestLikeService(CountingService::new(false)));
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let source = CountingService::reference(&context, 1);
    for name in ["DescribedBy", "Extends", "InstanceRelationships"] {
        source.related_holons(name)?;
        source.related_holons(name)?;
        assert_eq!(service.0.count(name), 1);
    }
    assert_eq!(
        service.0.calls.read().unwrap().len(),
        3,
        "no descriptor traversal needed for kernel edges"
    );
    Ok(())
}

#[test]
fn bounded_cache_expires_and_fresh_reads_replace_even_empty_membership() -> Result<(), HolonError> {
    use crate::RelationshipReadHint;
    let service = Arc::new(CountingService::new(false));
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = RelationshipCache::new();
    let id = HolonId::Local(LocalId(vec![1]));
    let name = "Moving".to_relationship_name();
    service.edges.write().unwrap().insert((1, "Moving".into()), vec![]);
    let read = |hint| {
        cache.related_holons_with_hint(&context, service.as_ref(), &id, &name, hint, || {
            Ok(RelationshipCachePolicy::MaxAgeMillis(30_000))
        })
    };
    let empty = read(RelationshipReadHint::SchemaDefault)?;
    service.edges.write().unwrap().insert((1, "Moving".into()), vec![11]);
    service.clock.store(29_999, Ordering::Relaxed);
    assert!(Arc::ptr_eq(&empty, &read(RelationshipReadHint::SchemaDefault)?));
    assert_eq!(service.count("Moving"), 1);
    service.clock.store(30_000, Ordering::Relaxed);
    let refreshed = read(RelationshipReadHint::SchemaDefault)?;
    assert_eq!(refreshed.read().unwrap().get_members().len(), 1);
    assert_eq!(service.count("Moving"), 2);
    service.edges.write().unwrap().insert((1, "Moving".into()), vec![]);
    let forced = read(RelationshipReadHint::RequireFresh)?;
    assert!(forced.read().unwrap().get_members().is_empty());
    assert!(Arc::ptr_eq(&forced, &read(RelationshipReadHint::SchemaDefault)?));
    assert_eq!(service.count("Moving"), 3);
    Ok(())
}

#[test]
fn guest_default_reuses_declared_definitional_membership() -> Result<(), HolonError> {
    for empty in [false, true] {
        let service = Arc::new(GuestLikeService(CountingService::new(false)));
        if empty {
            service.0.edges.write().unwrap().insert((1, "Frozen".into()), vec![]);
        }
        // An inherited contract must not trigger descriptor reads in the guest.
        service.0.edges.write().unwrap().remove(&(2, "InstanceRelationships".into()));
        service.0.edges.write().unwrap().insert((2, "Extends".into()), vec![20]);
        service
            .0
            .edges
            .write()
            .unwrap()
            .insert((20, "InstanceRelationships".into()), vec![5, 6, 12]);
        let space = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));
        let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let source = CountingService::reference(&context, 1);
        let first = source.related_holons("Frozen")?;
        let second = source.related_holons("Frozen")?;
        assert_eq!(
            service.0.count("Frozen"),
            1,
            "guest must retain declared definitional membership"
        );
        assert!(Arc::ptr_eq(&first, &second));
        for name in ["Moving", "Incoming", "Unknown"] {
            source.related_holons(name)?;
            source.related_holons(name)?;
            assert_eq!(service.0.count(name), 1, "all membership is reused within the request");
        }
        let fresh =
            source.related_holons_with_hint("Frozen", crate::RelationshipReadHint::RequireFresh)?;
        assert_eq!(service.0.count("Frozen"), 2);
        assert!(Arc::ptr_eq(&fresh, &source.related_holons("Frozen")?));
    }
    Ok(())
}

#[test]
fn one_shot_guest_relationship_read_does_not_fetch_a_descriptor_graph() -> Result<(), HolonError> {
    let service = Arc::new(GuestLikeService(CountingService::new(false)));
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    CountingService::reference(&context, 1).related_holons("Frozen")?;
    assert_eq!(
        service.0.calls.read().unwrap().values().sum::<usize>(),
        1,
        "guest read should fetch only the requested membership, without descriptor lookups"
    );
    Ok(())
}

#[test]
fn guest_reuses_all_membership_without_descriptors_and_honors_explicit_fresh(
) -> Result<(), HolonError> {
    for name in ["Moving", "Incoming", "Unknown"] {
        let service = Arc::new(GuestLikeService(CountingService::new(false)));
        let space = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));
        let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let source = CountingService::reference(&context, 1);
        let first = source.related_holons(name)?;
        service.0.edges.write().unwrap().insert((1, name.into()), vec![11]);
        let second = source.related_holons(name)?;
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            service.0.calls.read().unwrap().values().sum::<usize>(),
            1,
            "even repeated mutable or unknown reads must not consult descriptors"
        );
        let second =
            source.related_holons_with_hint(name, crate::RelationshipReadHint::RequireFresh)?;
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(
            second.read().unwrap().get_members(),
            &vec![CountingService::reference(&context, 11)]
        );
        assert_eq!(service.0.count(name), 2, "explicit fresh fetches again");
        service.0.edges.write().unwrap().insert((1, name.into()), vec![]);
        assert!(Arc::ptr_eq(&second, &source.related_holons(name)?));
        let next_request = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));
        let next_context =
            next_request.get_transaction_manager().open_public_transaction(next_request.clone())?;
        assert!(CountingService::reference(&next_context, 1)
            .related_holons(name)?
            .read()
            .unwrap()
            .get_members()
            .is_empty());
        assert_eq!(service.0.count(name), 3, "the next guest request starts empty");
    }
    Ok(())
}
