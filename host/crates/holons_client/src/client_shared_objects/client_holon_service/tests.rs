use super::*;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::{HolonCacheAccess, HolonCacheManager, ServiceRoutingPolicy};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use type_names::ToRelationshipName;

#[derive(Debug)]
struct CountingService {
    client: ClientHolonService,
    calls: AtomicUsize,
    policy_calls: AtomicUsize,
    metadata_reads: AtomicUsize,
    now: AtomicU64,
}
fn id(value: u8) -> HolonId {
    HolonId::Local(LocalId(vec![value; 39]))
}
impl CountingService {
    fn new() -> Self {
        Self {
            client: ClientHolonService::development_default(),
            calls: AtomicUsize::new(0),
            policy_calls: AtomicUsize::new(0),
            metadata_reads: AtomicUsize::new(0),
            now: AtomicU64::new(0),
        }
    }
    fn prepare(&self, source: u8, source_type: u8, name: &str, policy: RelationshipCachePolicy) {
        self.client.source_types.write().unwrap().insert(id(source), id(source_type));
        self.client
            .relationship_policies
            .write()
            .unwrap()
            .insert((id(source_type), name.to_relationship_name()), policy);
    }
}
impl HolonServiceApi for CountingService {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn relationship_cache_generation(&self) -> u64 {
        self.client.relationship_cache_generation()
    }
    fn relationship_cache_time_millis(&self) -> Option<u64> {
        Some(self.now.load(Ordering::Relaxed))
    }
    fn relationship_cache_policy(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<Option<RelationshipCachePolicy>, HolonError> {
        self.policy_calls.fetch_add(1, Ordering::Relaxed);
        self.client.relationship_cache_policy(context, source, name)
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<Holon, HolonError> {
        self.metadata_reads.fetch_add(1, Ordering::Relaxed);
        panic!("instance cache policy resolution must never fetch descriptor content")
    }
    fn fetch_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        _: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(HolonCollection::from_parts(
            holons_core::core_shared_objects::CollectionState::Fetched,
            vec![],
            Default::default(),
        ))
    }
    fn fetch_all_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        panic!("unexpected schema traversal")
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
fn fixture(
    service: &Arc<CountingService>,
) -> Result<(Arc<TransactionContext>, HolonCacheManager), HolonError> {
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    Ok((context, HolonCacheManager::new(service.clone())))
}

#[test]
fn first_and_expired_reads_resolve_policy_locally_and_retain_empty_membership(
) -> Result<(), HolonError> {
    for policy in [
        RelationshipCachePolicy::Reuse,
        RelationshipCachePolicy::Fresh,
        RelationshipCachePolicy::MaxAgeMillis(100),
    ] {
        let service = Arc::new(CountingService::new());
        service.prepare(1, 2, "Incoming", policy);
        let (context, cache) = fixture(&service)?;
        let name = "Incoming".to_relationship_name();
        let first = cache.get_related_holons(&context, &id(1), &name)?;
        let again = cache.get_related_holons(&context, &id(1), &name)?;
        assert_eq!(Arc::ptr_eq(&first, &again), policy != RelationshipCachePolicy::Fresh);
        service.now.store(200, Ordering::Relaxed);
        let expired = cache.get_related_holons(&context, &id(1), &name)?;
        assert_eq!(Arc::ptr_eq(&first, &expired), policy == RelationshipCachePolicy::Reuse);
        assert_eq!(service.policy_calls.load(Ordering::Relaxed), 1);
        assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    }
    Ok(())
}

#[test]
fn fresh_hint_bypasses_membership_without_recomputing_policy() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new());
    service.prepare(1, 2, "Incoming", RelationshipCachePolicy::MaxAgeMillis(100));
    let (context, cache) = fixture(&service)?;
    let name = "Incoming".to_relationship_name();
    let first = cache.get_related_holons(&context, &id(1), &name)?;
    let fresh = cache.get_related_holons_with_hint(
        &context,
        &id(1),
        &name,
        holons_core::RelationshipReadHint::RequireFresh,
    )?;
    assert!(!Arc::ptr_eq(&first, &fresh));
    assert!(Arc::ptr_eq(&fresh, &cache.get_related_holons(&context, &id(1), &name)?));
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), 1);
    assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    Ok(())
}

#[test]
fn missing_preparation_is_retryable_and_never_retained_as_authoritative_fresh(
) -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new());
    let (context, cache) = fixture(&service)?;
    let name = "Incoming".to_relationship_name();
    let unresolved = cache.get_related_holons(&context, &id(1), &name)?;
    service.prepare(1, 2, "Incoming", RelationshipCachePolicy::Reuse);
    let prepared = cache.get_related_holons(&context, &id(1), &name)?;
    assert!(!Arc::ptr_eq(&unresolved, &prepared));
    assert!(Arc::ptr_eq(&prepared, &cache.get_related_holons(&context, &id(1), &name)?));
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), 2);
    assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    Ok(())
}

#[test]
fn type_policies_are_shared_across_instances_and_names_remain_type_specific(
) -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new());
    service.prepare(1, 2, "Incoming", RelationshipCachePolicy::Reuse);
    service.client.source_types.write().unwrap().insert(id(3), id(2));
    service.prepare(4, 5, "Incoming", RelationshipCachePolicy::Fresh);
    let (context, cache) = fixture(&service)?;
    let name = "Incoming".to_relationship_name();
    for source in [1, 3, 4] {
        let first = cache.get_related_holons(&context, &id(source), &name)?;
        let again = cache.get_related_holons(&context, &id(source), &name)?;
        assert_eq!(Arc::ptr_eq(&first, &again), source != 4);
    }
    assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    Ok(())
}

#[test]
fn explicit_policy_adoption_invalidates_membership_and_policy_together() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new());
    service.prepare(1, 2, "Incoming", RelationshipCachePolicy::Reuse);
    let (context, cache) = fixture(&service)?;
    let name = "Incoming".to_relationship_name();
    let first = cache.get_related_holons(&context, &id(1), &name)?;
    service.prepare(1, 2, "Incoming", RelationshipCachePolicy::Fresh);
    service.client.prepared_generation.fetch_add(1, Ordering::AcqRel);
    let changed = cache.get_related_holons(&context, &id(1), &name)?;
    assert!(!Arc::ptr_eq(&first, &changed));
    assert!(!Arc::ptr_eq(&changed, &cache.get_related_holons(&context, &id(1), &name)?));
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), 2);
    assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    Ok(())
}
