use super::*;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::{HolonCacheAccess, HolonCacheManager, ServiceRoutingPolicy};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;
use type_names::ToRelationshipName;

#[derive(Debug)]
struct CountingService {
    client: ClientHolonService,
    // Stand-in provider values exercise the cache boundary, not production schema resolution.
    policies: RwLock<HashMap<(HolonId, RelationshipName), RelationshipCachePolicy>>,
    bind_on_fetch: AtomicBool,
    binding_calls: AtomicUsize,
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
            policies: Default::default(),
            bind_on_fetch: AtomicBool::new(false),
            binding_calls: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
            policy_calls: AtomicUsize::new(0),
            metadata_reads: AtomicUsize::new(0),
            now: AtomicU64::new(0),
        }
    }
    fn prepare(&self, source: u8, name: &str, policy: RelationshipCachePolicy) {
        self.policies.write().unwrap().insert((id(source), name.to_relationship_name()), policy);
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
        Ok(self.policies.read().unwrap().get(&(source.clone(), name.clone())).copied())
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        source: &HolonId,
    ) -> Result<Holon, HolonError> {
        self.metadata_reads.fetch_add(1, Ordering::Relaxed);
        let HolonId::Local(local) = source else { panic!("local fixture identity") };
        use type_names::ToPropertyName;
        Ok(Holon::Saved(holons_core::core_shared_objects::holon::SavedHolon::new(
            local.clone(),
            core_types::PropertyMap::from([(
                "Key".to_property_name(),
                base_types::BaseValue::StringValue("undescribed".into()),
            )]),
            None,
            base_types::MapInteger(1),
        )))
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
    fn fetch_related_holons_with_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<holons_core::core_shared_objects::relationship_fetch::FetchedRelationship, HolonError>
    {
        use holons_core::core_shared_objects::relationship_fetch::{
            FetchedRelationship, RelationshipPolicyBinding,
        };
        let collection = self.fetch_related_holons_internal(context, source, name)?;
        let descriptor = if self.bind_on_fetch.load(Ordering::Relaxed) {
            self.binding_calls.fetch_add(1, Ordering::Relaxed);
            Some(RelationshipPolicyBinding {
                descriptor_id: id(9),
                policy: *self
                    .policies
                    .read()
                    .unwrap()
                    .get(&(source.clone(), name.clone()))
                    .unwrap(),
            })
        } else {
            None
        };
        Ok(FetchedRelationship { collection, descriptor })
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
fn client_policy_lookup_never_discovers_schema() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new());
    let (context, _) = fixture(&service)?;
    for name in ["DescribedBy", "Extends", "InstanceRelationships"] {
        assert_eq!(
            service.client.relationship_cache_policy(
                &context,
                &id(1),
                &name.to_relationship_name()
            )?,
            Some(RelationshipCachePolicy::Reuse)
        );
    }
    assert_eq!(service.calls.load(Ordering::Relaxed), 0);
    assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    assert_eq!(
        service.client.relationship_cache_policy(
            &context,
            &id(1),
            &"ValueType".to_relationship_name()
        )?,
        None
    );
    assert_eq!(service.calls.load(Ordering::Relaxed), 0);
    assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    Ok(())
}

#[derive(Debug, Default)]
struct MembershipTransport {
    fresh: AtomicBool,
    descriptor_requests: std::sync::Mutex<Vec<bool>>,
}

#[async_trait::async_trait]
impl holons_core::dances::DanceInitiator for MembershipTransport {
    async fn initiate_dance(
        &self,
        context: &Arc<TransactionContext>,
        request: holons_core::dances::DanceRequest,
    ) -> holons_core::dances::DanceResponse {
        use holons_core::core_shared_objects::relationship_fetch::{
            FetchedRelationship, RelationshipPolicyBinding,
        };
        use holons_core::dances::{DanceResponse, RequestBody};
        assert_eq!(request.dance_name.0, "query_relationships");
        // Use the same request and response projection/JSON/binding as transport.
        let wire = holons_boundary::DanceRequestWire::from(&request);
        let json = serde_json::to_vec(&wire).unwrap();
        let request: holons_boundary::DanceRequestWire = serde_json::from_slice(&json).unwrap();
        let request = request.bind(context).unwrap();
        let RequestBody::QueryExpression(expression) = request.body else {
            panic!("membership query");
        };
        assert_eq!(expression.relationship_name, "ValueType".to_relationship_name());
        self.descriptor_requests.lock().unwrap().push(expression.include_relationship_descriptor);
        let body = if expression.include_relationship_descriptor {
            ResponseBody::SavedRelationship(FetchedRelationship {
                collection: HolonCollection::new_existing(),
                descriptor: Some(RelationshipPolicyBinding {
                    descriptor_id: id(9),
                    policy: if self.fresh.load(Ordering::Relaxed) {
                        RelationshipCachePolicy::Fresh
                    } else {
                        RelationshipCachePolicy::Reuse
                    },
                }),
            })
        } else {
            ResponseBody::NodeCollection(NodeCollection::new_empty())
        };
        let response = DanceResponse::new(ResponseStatusCode::OK, "Success".into(), body, None);
        let wire = holons_boundary::DanceResponseWire::from(&response);
        let json = serde_json::to_vec(&wire).unwrap();
        let wire: holons_boundary::DanceResponseWire = serde_json::from_slice(&json).unwrap();
        wire.bind(context).unwrap()
    }
}

#[test]
fn client_cache_binds_policy_in_one_membership_request_and_reuses_it_for_fresh_reads(
) -> Result<(), HolonError> {
    let service = Arc::new(ClientHolonService::development_default());
    let transport = Arc::new(MembershipTransport::default());
    let space = HolonSpaceManager::new_with_managers(
        Some(transport.clone()),
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let _context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = HolonCacheManager::new(service, Arc::downgrade(&space));
    let name = "ValueType".to_relationship_name();
    let first = cache.get_related_holons(&id(1), &name)?;
    assert!(Arc::ptr_eq(&first, &cache.get_related_holons(&id(1), &name)?));
    let fresh = cache.get_related_holons_with_hint(
        &id(1),
        &name,
        holons_core::RelationshipReadHint::RequireFresh,
    )?;
    assert!(!Arc::ptr_eq(&first, &fresh));
    assert!(Arc::ptr_eq(&fresh, &cache.get_related_holons(&id(1), &name)?));
    assert_eq!(*transport.descriptor_requests.lock().unwrap(), vec![true, false]);
    Ok(())
}

#[test]
fn bulk_preparation_keeps_descriptor_binding_for_fresh_membership() -> Result<(), HolonError> {
    let service = Arc::new(ClientHolonService::development_default());
    let transport = Arc::new(MembershipTransport::default());
    transport.fresh.store(true, Ordering::Relaxed);
    let space = HolonSpaceManager::new_with_managers(
        Some(transport.clone()),
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let _context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = HolonCacheManager::new(service, Arc::downgrade(&space));
    let name = "ValueType".to_relationship_name();
    cache.prepare_relationships(&id(1), &[name.clone()])?;
    cache.prepare_relationships(&id(1), &[name.clone()])?;
    cache.get_related_holons(&id(1), &name)?;
    assert_eq!(*transport.descriptor_requests.lock().unwrap(), vec![true, false, false]);
    Ok(())
}

#[test]
fn fetched_descriptor_policy_needs_no_local_schema_discovery_and_survives_expiry(
) -> Result<(), HolonError> {
    for policy in [
        RelationshipCachePolicy::Reuse,
        RelationshipCachePolicy::Fresh,
        RelationshipCachePolicy::MaxAgeMillis(100),
    ] {
        let service = Arc::new(CountingService::new());
        service.bind_on_fetch.store(true, Ordering::Relaxed);
        service.prepare(1, "ValueType", policy);
        let (_context, cache) = fixture(&service)?;
        let name = "ValueType".to_relationship_name();
        cache.get_related_holons(&id(1), &name)?;
        cache.get_related_holons(&id(1), &name)?;
        service.now.store(200, Ordering::Relaxed);
        cache.get_related_holons(&id(1), &name)?;
        assert_eq!(service.binding_calls.load(Ordering::Relaxed), 1);
        assert_eq!(service.policy_calls.load(Ordering::Relaxed), 0);
        assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
        let expected_fetches = match policy {
            RelationshipCachePolicy::Reuse => 1,
            RelationshipCachePolicy::Fresh => 3,
            _ => 2,
        };
        assert_eq!(service.calls.load(Ordering::Relaxed), expected_fetches);
    }
    Ok(())
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
        service.prepare(1, "Incoming", policy);
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
    service.prepare(1, "Incoming", RelationshipCachePolicy::MaxAgeMillis(100));
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
    service.prepare(1, "Incoming", RelationshipCachePolicy::Reuse);
    let prepared = cache.get_related_holons(&id(1), &name)?;
    assert!(!Arc::ptr_eq(&unresolved, &prepared));
    assert!(Arc::ptr_eq(&prepared, &cache.get_related_holons(&id(1), &name)?));
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), 2);
    assert_eq!(service.metadata_reads.load(Ordering::Relaxed), 0);
    Ok(())
}

#[test]
fn cache_entries_preserve_source_specific_retention() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new());
    service.prepare(1, "Incoming", RelationshipCachePolicy::Reuse);
    service.prepare(3, "Incoming", RelationshipCachePolicy::Reuse);
    service.prepare(4, "Incoming", RelationshipCachePolicy::Fresh);
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
    service.prepare(1, "Incoming", RelationshipCachePolicy::Reuse);
    let (context, cache) = fixture(&service)?;
    let name = "Incoming".to_relationship_name();
    let first = cache.get_related_holons(&id(1), &name)?;
    service.prepare(1, "Incoming", RelationshipCachePolicy::Fresh);
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
    evidence: Option<base_types::BaseValue>,
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
        if self.status == "TransportFailure" {
            return holons_core::dances::DanceResponse::from_error(HolonError::ConductorError(
                "unknown persistence outcome".into(),
            ));
        }
        if self.status == "InvalidResponse" {
            return holons_core::dances::DanceResponse::new(
                ResponseStatusCode::OK,
                "missing body".into(),
                ResponseBody::None,
                None,
            );
        }
        let mut response = context.mutation().new_holon(None).unwrap();
        if let Some(evidence) = &self.evidence {
            response
                .with_property_value(
                    type_names::CorePropertyTypeName::DescriptorResetRequired,
                    evidence.clone(),
                )
                .unwrap();
        }
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
    use base_types::BaseValue;
    for (status, persisted, reset_during_request, evidence, nonempty, should_reset) in [
        ("Complete", true, false, "missing", false, true),
        ("Complete", true, true, "reset", false, true),
        ("Complete", false, false, "missing", false, false),
        ("Complete", false, false, "missing", true, true),
        ("Complete", true, false, "safe", true, false),
        ("Complete", true, true, "safe", true, false),
        ("Complete", false, false, "safe", true, false),
        ("Complete", true, false, "malformed", true, true),
        ("Complete", false, false, "reset", false, true),
        ("Rejected", false, false, "reset", true, false),
        ("Rejected", true, false, "safe", true, true),
        ("Incomplete", true, false, "safe", true, true),
        // Partial graph persistence can occur without a newly saved node.
        ("Incomplete", false, false, "safe", false, true),
        ("Unknown", false, false, "safe", false, true),
        ("TransportFailure", false, false, "safe", true, true),
        ("InvalidResponse", false, false, "safe", true, true),
    ] {
        let evidence = match evidence {
            "safe" => Some(BaseValue::BooleanValue(false.into())),
            "reset" => Some(BaseValue::BooleanValue(true.into())),
            "malformed" => Some(BaseValue::StringValue("false".into())),
            _ => None,
        };
        let transport = Arc::new(CommitResetTransport {
            status,
            evidence,
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
        let candidates = if nonempty {
            let transient = context.mutation().new_holon(Some("candidate".into())).unwrap();
            vec![context.mutation().stage_new_holon(transient).unwrap()]
        } else {
            vec![]
        };
        let before = space.session().unwrap();
        let result = client.commit_internal(&context, &candidates);
        match status {
            "TransportFailure" => assert!(matches!(result, Err(HolonError::ConductorError(_)))),
            "InvalidResponse" => assert!(matches!(result, Err(HolonError::CommitFailure(_)))),
            _ => {
                result.unwrap();
            }
        }
        let during = transport.during.lock().unwrap().clone().unwrap();
        assert_eq!(during.check_active().is_err(), should_reset, "{status}/{persisted}");
        assert_eq!(before.check_active().is_err(), should_reset || reset_during_request);
        assert!(space.session().unwrap().check_active().is_ok());
    }
}

#[test]
fn derived_reset_preserves_membership_and_its_retained_scalar_policy() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new());
    service.prepare(1, "Frozen", RelationshipCachePolicy::Reuse);
    let (context, cache) = fixture(&service)?;
    let name = "Frozen".to_relationship_name();
    let before = cache.get_related_holons(&id(1), &name)?;
    let old = context.space_read_handle().session()?;
    context.space_read_handle().reset_session()?;
    assert_eq!(old.check_active(), Err(HolonError::ObsoleteSession));
    assert!(Arc::ptr_eq(&before, &cache.get_related_holons(&id(1), &name)?));
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), 1);
    Ok(())
}
