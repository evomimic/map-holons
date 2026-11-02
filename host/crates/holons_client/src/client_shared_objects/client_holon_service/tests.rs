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
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = HolonCacheManager::new(service.clone(), Arc::downgrade(&space));
    Ok((context, cache))
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
        let first = cache.get_related_holons(&id(1), &name)?;
        let again = cache.get_related_holons(&id(1), &name)?;
        assert_eq!(Arc::ptr_eq(&first, &again), policy != RelationshipCachePolicy::Fresh);
        service.now.store(200, Ordering::Relaxed);
        let expired = cache.get_related_holons(&id(1), &name)?;
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
    let first = cache.get_related_holons(&id(1), &name)?;
    let fresh = cache.get_related_holons_with_hint(
        &id(1),
        &name,
        holons_core::RelationshipReadHint::RequireFresh,
    )?;
    assert!(!Arc::ptr_eq(&first, &fresh));
    assert!(Arc::ptr_eq(&fresh, &cache.get_related_holons(&id(1), &name)?));
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
    let unresolved = cache.get_related_holons(&id(1), &name)?;
    service.prepare(1, 2, "Incoming", RelationshipCachePolicy::Reuse);
    let prepared = cache.get_related_holons(&id(1), &name)?;
    assert!(!Arc::ptr_eq(&unresolved, &prepared));
    assert!(Arc::ptr_eq(&prepared, &cache.get_related_holons(&id(1), &name)?));
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
        let first = cache.get_related_holons(&id(source), &name)?;
        let again = cache.get_related_holons(&id(source), &name)?;
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
    let first = cache.get_related_holons(&id(1), &name)?;
    service.prepare(1, 2, "Incoming", RelationshipCachePolicy::Fresh);
    service.client.prepared_generation.fetch_add(1, Ordering::AcqRel);
    let changed = cache.get_related_holons(&id(1), &name)?;
    assert!(!Arc::ptr_eq(&first, &changed));
    assert!(!Arc::ptr_eq(&changed, &cache.get_related_holons(&id(1), &name)?));
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), 2);
    assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    Ok(())
}

#[derive(Debug, Default)]
struct UsageResolutionTransport {
    calls: AtomicUsize,
    ambiguous: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl holons_core::dances::DanceInitiator for UsageResolutionTransport {
    async fn initiate_dance(
        &self,
        context: &Arc<TransactionContext>,
        request: holons_core::dances::DanceRequest,
    ) -> holons_core::dances::DanceResponse {
        use holons_core::dances::{DanceResponse, RequestBody};
        use holons_core::WritableHolon;
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(request.dance_name.0, "resolve_visualizer_usage");
        let RequestBody::TargetHolons(name, inputs) = request.body else {
            panic!("resolution inputs")
        };
        assert_eq!(name.to_string(), "UsageResolutionInputs");
        assert_eq!(inputs.len(), 3);
        if self.ambiguous.load(Ordering::Relaxed) {
            return DanceResponse::from_error(HolonError::InvalidState(
                "Multiple applicable VisualizerUsages".into(),
            ));
        }
        let mut projection =
            context.mutation().new_holon(Some("usage-resolution-response".into())).unwrap();
        projection.add_related_holons("DefaultUsageType", vec![inputs[1].clone()]).unwrap();
        projection.add_related_holons("UsageSearchPath", vec![inputs[1].clone()]).unwrap();
        let response = DanceResponse::new(
            ResponseStatusCode::OK,
            "Success".into(),
            ResponseBody::HolonReference(projection.into()),
            None,
        );
        // Exercise the existing boundary projection/binding used by real service responses.
        holons_boundary::DanceResponseWire::from(&response).bind(context).unwrap()
    }
}

#[test]
fn usage_resolution_delegates_once_without_client_relationship_discovery() -> Result<(), HolonError>
{
    let service = Arc::new(CountingService::new());
    let transport = Arc::new(UsageResolutionTransport::default());
    let space = HolonSpaceManager::new_with_managers(
        Some(transport.clone()),
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let visualizer =
        HolonReference::smart_with_key(context.space_read_handle(), id(1), "node-1".into());
    let subject =
        HolonReference::smart_with_key(context.space_read_handle(), id(2), "node-2".into());
    let slot = HolonReference::smart_with_key(context.space_read_handle(), id(3), "node-3".into());
    let result =
        service.client.resolve_visualizer_usage_internal(&context, &visualizer, &subject, &slot)?;
    assert!(result.usage.is_none());
    assert_eq!(result.default_type, subject);
    assert_eq!(result.search_path, vec![subject.clone()]);
    assert_eq!(transport.calls.load(Ordering::Relaxed), 1);
    assert_eq!(service.calls.load(Ordering::Relaxed), 0);
    assert_eq!(context.lookup().staged_count()?, 0);
    transport.ambiguous.store(true, Ordering::Relaxed);
    assert!(matches!(
        service.client.resolve_visualizer_usage_internal(&context, &visualizer, &subject, &slot),
        Err(HolonError::InvalidState(_))
    ));
    assert_eq!(transport.calls.load(Ordering::Relaxed), 2);
    Ok(())
}

#[derive(Debug)]
struct CommitResetTransport {
    status: &'static str,
    persisted: bool,
    reset_during_request: bool,
    during: std::sync::Mutex<
        Option<Arc<holons_core::core_shared_objects::space_session::SpaceSession>>,
    >,
}

#[async_trait::async_trait]
impl holons_core::dances::DanceInitiator for CommitResetTransport {
    async fn initiate_dance(
        &self,
        context: &Arc<TransactionContext>,
        _: holons_core::dances::DanceRequest,
    ) -> holons_core::dances::DanceResponse {
        use holons_core::WritableHolon;
        if self.reset_during_request {
            context.space_read_handle().reset_session().unwrap();
        }
        *self.during.lock().unwrap() = Some(context.space_read_handle().session().unwrap());
        let mut response = context.mutation().new_holon(None).unwrap();
        response
            .with_property_value(type_names::CorePropertyTypeName::CommitRequestStatus, self.status)
            .unwrap();
        let members = if self.persisted {
            vec![context.mutation().new_holon(Some("persisted-result".into())).unwrap().into()]
        } else {
            vec![]
        };
        response
            .add_related_holons(type_names::CoreRelationshipTypeName::SavedHolons, members)
            .unwrap();
        holons_core::dances::DanceResponse::new(
            ResponseStatusCode::OK,
            "Commit completed".into(),
            ResponseBody::HolonReference(response.into()),
            None,
        )
    }
}

#[test]
fn local_commit_resets_current_session_only_when_persistence_may_have_changed() {
    for (status, persisted, reset_during_request, should_reset) in [
        ("Complete", true, false, true),
        ("Complete", true, true, true),
        ("Complete", false, false, false),
        ("Rejected", false, false, false),
        ("Incomplete", true, false, true),
        // Partial graph persistence can occur without a newly saved node.
        ("Incomplete", false, false, true),
    ] {
        let transport = Arc::new(CommitResetTransport {
            status,
            persisted,
            reset_during_request,
            during: Default::default(),
        });
        let client = Arc::new(ClientHolonService::development_default());
        let space = HolonSpaceManager::new_with_managers(
            Some(transport.clone()),
            client.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        );
        let context =
            space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
        let before = space.session().unwrap();
        client.commit_internal(&context, &[]).unwrap();
        let during = transport.during.lock().unwrap().clone().unwrap();
        assert_eq!(during.check_active().is_err(), should_reset, "{status}/{persisted}");
        assert_eq!(before.check_active().is_err(), should_reset || reset_during_request);
        assert!(space.session().unwrap().check_active().is_ok());
    }
}

#[test]
fn derived_reset_preserves_policies_source_resolutions_and_membership_generation() {
    let client = Arc::new(ClientHolonService::development_default());
    client.source_types.write().unwrap().insert(id(1), id(2));
    client
        .relationship_policies
        .write()
        .unwrap()
        .insert((id(2), "Frozen".to_relationship_name()), RelationshipCachePolicy::Reuse);
    let generation = client.relationship_cache_generation();
    let space = HolonSpaceManager::new_with_managers(
        None,
        client.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let old = space.session().unwrap();
    space.reset_session().unwrap();
    assert_eq!(old.check_active(), Err(HolonError::ObsoleteSession));
    assert_eq!(client.source_types.read().unwrap().get(&id(1)), Some(&id(2)));
    assert_eq!(
        client.relationship_policies.read().unwrap().get(&(id(2), "Frozen".to_relationship_name())),
        Some(&RelationshipCachePolicy::Reuse)
    );
    assert_eq!(client.relationship_cache_generation(), generation);
}
