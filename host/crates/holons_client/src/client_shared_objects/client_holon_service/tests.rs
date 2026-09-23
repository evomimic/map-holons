use super::*;
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_api::core_shared_objects::{holon::SavedHolon, space_manager::HolonSpaceManager};
use core_api::RelationshipCachePolicy;
use core_api::{HolonCollectionApi, ServiceRoutingPolicy, StagedReference, TransientReference};
use core_types::{LocalId, PropertyMap, PropertyName};
use holons_core as core_api;
use holons_core::{HolonCacheAccess, HolonCacheManager, RelationshipCache};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::{any::Any, collections::HashMap};
use type_names::ToRelationshipName;

#[derive(Debug)]
struct CountingService {
    client: ClientHolonService,
    edges: RwLock<HashMap<(u8, String), Vec<u8>>>,
    calls: RwLock<HashMap<(u8, String), usize>>,
    bulk_calls: AtomicUsize,
    inverse_cache_age: Option<i64>,
    now: AtomicU64,
}

impl CountingService {
    fn new(_reuse_frozen: bool) -> Self {
        let mut edges = HashMap::new();
        for (source, name, targets) in [
            (1, "DescribedBy", vec![2]),
            (2, "InstanceRelationships", vec![5, 6, 12]),
            (2, "Extends", vec![17]),
            (17, "SourceOf", vec![7]),
            (7, "SourceType", vec![17]),
            (5, "Extends", vec![3]),
            (6, "Extends", vec![3]),
            (7, "Extends", vec![4]),
            (9, "Extends", vec![3]),
            (12, "Extends", vec![3]),
            (8, "InstanceRelationships", vec![9]),
            (9, "SourceType", vec![8]),
            (9, "TargetType", vec![2]),
            (9, "HasInverse", vec![7]),
            (9, "DescribedBy", vec![13]),
            (13, "InstanceRelationships", vec![14, 15, 16]),
            (14, "Extends", vec![3]),
            (15, "Extends", vec![3]),
            (16, "Extends", vec![3]),
            (1, "Frozen", vec![10]),
            (1, "Moving", vec![10]),
            (1, "Incoming", vec![10]),
        ] {
            edges.insert((source, name.into()), targets);
        }
        Self {
            client: ClientHolonService::development_default(),
            edges: RwLock::new(edges),
            calls: RwLock::new(HashMap::new()),
            bulk_calls: AtomicUsize::new(0),
            inverse_cache_age: None,
            now: AtomicU64::new(0),
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
        Some(self.now.load(Ordering::Relaxed))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn relationship_cache_policy(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<RelationshipCachePolicy, HolonError> {
        self.client.relationship_cache_policy(context, source, name)
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
            7 | 21 => "Incoming",
            9 => "Outgoing",
            12 => "DescribedBy",
            14 => "SourceType",
            15 => "TargetType",
            16 => "HasInverse",
            _ => "Type",
        };
        let mut properties = PropertyMap::new();
        if number == 7 {
            if let Some(age) = self.inverse_cache_age {
                properties.insert(
                    PropertyName("MembershipCacheMaxAgeMillis".into()),
                    BaseValue::IntegerValue(MapInteger(age)),
                );
            }
        }
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
            let _guard = service.client.enter_relationship_semantics_resolution();
            let hit = manager.get_related_holons(&context, &id, &name)?;
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

#[test]
fn inverse_descriptor_selects_bounded_reuse_and_fresh_hint_bypasses_it() -> Result<(), HolonError> {
    let mut service = CountingService::new(false);
    service.inverse_cache_age = Some(30_000);
    let service = Arc::new(service);
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let source = CountingService::reference(&context, 1);
    let first = source.related_holons("Incoming")?;
    assert!(Arc::ptr_eq(&first, &source.related_holons("Incoming")?));
    assert_eq!(service.count("Incoming"), 1);
    service.edges.write().unwrap().insert((1, "Incoming".into()), vec![11]);
    let fresh = source
        .related_holons_with_hint("Incoming", holons_core::RelationshipReadHint::RequireFresh)?;
    assert_eq!(
        fresh.read().unwrap().get_members(),
        &vec![CountingService::reference(&context, 11)]
    );
    assert!(Arc::ptr_eq(&fresh, &source.related_holons("Incoming")?));
    assert_eq!(service.count("Incoming"), 2);
    Ok(())
}

#[test]
fn negative_inverse_cache_age_is_rejected() -> Result<(), HolonError> {
    let mut service = CountingService::new(false);
    service.inverse_cache_age = Some(-1);
    let service = Arc::new(service);
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    assert!(matches!(
        CountingService::reference(&context, 1).related_holons("Incoming"),
        Err(HolonError::InvalidParameter(_))
    ));
    Ok(())
}

#[test]
fn inverse_policy_uses_source_ancestry_without_target_discovery() -> Result<(), HolonError> {
    let mut service = CountingService::new(false);
    service.inverse_cache_age = Some(30_000);
    let service = Arc::new(service);
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let source = CountingService::reference(&context, 1);
    let first = source.related_holons("Incoming")?;
    assert!(Arc::ptr_eq(&first, &source.related_holons("Incoming")?));
    assert_eq!(service.count("Incoming"), 1);
    for ((_, name), _) in service.calls.read().unwrap().iter() {
        assert!(
            !["TargetOf", "TargetType", "HasInverse", "InverseOf"].contains(&name.as_str()),
            "policy lookup must not use target-side discovery: {name}"
        );
    }
    Ok(())
}

#[test]
fn cold_definitional_membership_is_cached_during_inverse_resolution() -> Result<(), HolonError> {
    for empty in [false, true] {
        let service = Arc::new(CountingService::new(false));
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
        let source = CountingService::reference(&context, 1);
        let first = {
            let _guard = service.client.enter_relationship_semantics_resolution();
            let first = source.related_holons("Frozen")?;
            assert!(Arc::ptr_eq(&first, &source.related_holons("Frozen")?));
            first
        };
        let next = space.get_transaction_manager().open_public_transaction(space.clone())?;
        assert!(Arc::ptr_eq(
            &first,
            &CountingService::reference(&next, 1).related_holons("Frozen")?
        ));
        assert_eq!(
            service.count("Frozen"),
            1,
            "a cold definitional read must be retained even while another policy is resolving"
        );
    }
    Ok(())
}

#[test]
fn resolved_inverse_descriptor_survives_expiry_and_is_shared_by_source_type(
) -> Result<(), HolonError> {
    let mut service = CountingService::new(false);
    service.inverse_cache_age = Some(30_000);
    service
        .edges
        .write()
        .unwrap()
        .extend([((18, "DescribedBy".into()), vec![2]), ((18, "Incoming".into()), vec![11])]);
    let service = Arc::new(service);
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let source = CountingService::reference(&context, 1);
    source.related_holons("Incoming")?;
    let schema_reads = || {
        service
            .calls
            .read()
            .unwrap()
            .iter()
            .filter(|((_, name), _)| name == "SourceOf")
            .map(|(_, count)| count)
            .sum::<usize>()
    };
    let before = schema_reads();
    assert!(before > 0);
    service.edges.write().unwrap().insert((1, "Incoming".into()), vec![11]);
    service.now.store(30_000, Ordering::Relaxed);
    let refreshed = source.related_holons("Incoming")?;
    assert_eq!(service.count("Incoming"), 2, "membership must refresh at the expiry boundary");
    assert_eq!(
        refreshed.read().unwrap().get_members(),
        &vec![CountingService::reference(&context, 11)]
    );
    assert_eq!(schema_reads(), before, "expiry must not rediscover the inverse descriptor");
    let next = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let other = CountingService::reference(&next, 18);
    other.related_holons("Incoming")?;
    assert_eq!(schema_reads(), before, "another source of the same type must reuse resolution");
    source.related_holons_with_hint("Incoming", holons_core::RelationshipReadHint::RequireFresh)?;
    assert_eq!(service.count("Incoming"), 3);
    assert_eq!(schema_reads(), before, "RequireFresh refreshes membership, not schema");
    Ok(())
}

#[test]
fn fresh_membership_still_reuses_successful_descriptor_resolution() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(false));
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let source = CountingService::reference(&context, 1);
    source.related_holons("Incoming")?;
    let before = service.calls.read().unwrap().clone();
    source.related_holons("Incoming")?;
    assert_eq!(service.count("Incoming"), 2);
    for (key, count) in before {
        if key.1 == "SourceOf" {
            assert_eq!(service.calls.read().unwrap()[&key], count);
        }
    }
    Ok(())
}

#[test]
fn same_relationship_name_on_different_types_keeps_distinct_policies() -> Result<(), HolonError> {
    let mut service = CountingService::new(false);
    service.inverse_cache_age = Some(30_000);
    service.edges.write().unwrap().extend([
        ((19, "DescribedBy".into()), vec![20]),
        ((20, "SourceOf".into()), vec![21]),
        ((21, "SourceType".into()), vec![20]),
        ((21, "Extends".into()), vec![4]),
        ((19, "Incoming".into()), vec![10]),
    ]);
    let service = Arc::new(service);
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let bounded = CountingService::reference(&context, 1);
    let fresh = CountingService::reference(&context, 19);
    bounded.related_holons("Incoming")?;
    fresh.related_holons("Incoming")?;
    bounded.related_holons("Incoming")?;
    fresh.related_holons("Incoming")?;
    assert_eq!(service.count("Incoming"), 1);
    assert_eq!(service.calls.read().unwrap()[&(19, "Incoming".into())], 2);
    Ok(())
}

#[test]
fn recursive_fallback_does_not_poison_later_inverse_resolution() -> Result<(), HolonError> {
    let mut service = CountingService::new(false);
    service.inverse_cache_age = Some(30_000);
    let service = Arc::new(service);
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let source = CountingService::reference(&context, 1);
    {
        let _guard = service.client.enter_relationship_semantics_resolution();
        source.related_holons("Incoming")?;
    }
    source.related_holons("Incoming")?;
    source.related_holons("Incoming")?;
    assert_eq!(service.count("Incoming"), 2, "fallback must not retain a false Fresh policy");
    Ok(())
}
