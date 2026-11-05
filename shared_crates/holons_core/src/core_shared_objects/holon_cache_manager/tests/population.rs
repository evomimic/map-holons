use super::*;
use crate::core_shared_objects::cache_population::SavedHolonFetch;

fn id(n: u8) -> HolonId {
    HolonId::Local(LocalId(vec![n]))
}
fn content(n: u8) -> Holon {
    Holon::Saved(SavedHolon::new(LocalId(vec![n]), PropertyMap::new(), None, MapInteger(1)))
}
fn setup() -> (Arc<CountingService>, Arc<HolonSpaceManager>, Arc<TransactionContext>) {
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    (service, space, context)
}

/// Assert reuse through ordinary resolution without exposing a residency query.
fn resolve_without_fetch(
    service: &CountingService,
    context: &Arc<TransactionContext>,
    id: &HolonId,
) -> Arc<RwLock<Holon>> {
    let before = service.content_reads.load(Ordering::Relaxed);
    let holon = context.space_read_handle().get_rc_holon(id).unwrap();
    assert_eq!(service.content_reads.load(Ordering::Relaxed), before);
    holon
}

#[test]
fn cold_partial_warm_duplicates_empty_and_cross_request_reuse() {
    for initially_warm in [0, 54, 64] {
        let (service, space, context) = setup();
        let ids: Vec<_> = (1..=64).map(id).collect();
        if initially_warm != 0 {
            assert!(context.ensure_cached(&ids[..initially_warm]).is_empty());
        }
        service.hydration_requests.write().unwrap().clear();
        let existing = HolonReference::smart_from_id(context.space_read_handle(), id(64));
        let mut duplicates = ids.clone();
        duplicates.extend(ids.clone());
        assert!(context.ensure_cached(&duplicates).is_empty());
        let calls = service.hydration_requests.read().unwrap().clone();
        if initially_warm == 64 {
            assert!(calls.is_empty());
        } else {
            assert_eq!(calls, vec![ids[initially_warm..].to_vec()]);
        }
        let reads = service.content_reads.load(Ordering::Relaxed);
        existing.key().unwrap();
        let next = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
        assert!(next.ensure_cached(&ids).is_empty());
        assert!(next.ensure_cached(&[]).is_empty());
        assert_eq!(service.content_reads.load(Ordering::Relaxed), reads);
        assert_eq!(service.hydration_requests.read().unwrap().len(), calls.len());
        assert!(context.is_open());
        assert_eq!(context.lookup().staged_count().unwrap(), 0);
    }
}

#[test]
fn per_id_absence_failure_and_transport_failure_do_not_erase_successes() {
    let (service, _space, context) = setup();
    *service.hydration_response.write().unwrap() = Some(Ok(vec![
        SavedHolonFetch { id: id(3), result: Err(HolonError::InvalidParameter("bad ID".into())) },
        SavedHolonFetch { id: id(1), result: Ok(content(1)) },
        SavedHolonFetch { id: id(2), result: Err(HolonError::HolonNotFound("missing".into())) },
    ]));
    let errors = context.ensure_cached(&[id(1), id(2), id(3)]);
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].ids, vec![id(2)]);
    assert!(matches!(errors[0].error, HolonError::HolonNotFound(_)));
    assert_eq!(errors[1].ids, vec![id(3)]);
    resolve_without_fetch(&service, &context, &id(1));
    *service.hydration_response.write().unwrap() =
        Some(Err(HolonError::ConductorError("offline".into())));
    let errors = context.ensure_cached(&[id(1), id(2), id(3)]);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].ids, vec![id(2), id(3)]);
    resolve_without_fetch(&service, &context, &id(1));
    *service.hydration_response.write().unwrap() = None;
    assert!(context.ensure_cached(&[id(2), id(3)]).is_empty(), "failures are not negative cached");
}

#[test]
fn invalid_associations_are_errors_while_independent_successes_are_cached() {
    let (service, _space, context) = setup();
    *service.hydration_response.write().unwrap() = Some(Ok(vec![
        SavedHolonFetch { id: id(1), result: Ok(content(1)) },
        SavedHolonFetch { id: id(1), result: Ok(content(1)) },
        // 2 omitted; 3 has mismatched content; 4 is not saved; 9 is unexpected.
        SavedHolonFetch { id: id(3), result: Ok(content(8)) },
        SavedHolonFetch {
            id: id(4),
            result: Ok(Holon::Transient(crate::core_shared_objects::TransientHolon::from_parts(
                MapInteger(1),
                crate::core_shared_objects::holon::HolonState::Mutable,
                crate::core_shared_objects::holon::ValidationState::ValidationRequired,
                PropertyMap::new(),
                crate::core_shared_objects::TransientRelationshipMap::new_empty(),
                None,
            ))),
        },
        SavedHolonFetch { id: id(9), result: Ok(content(9)) },
        SavedHolonFetch { id: id(5), result: Ok(content(5)) },
    ]));
    let errors = context.ensure_cached(&[id(1), id(2), id(3), id(4), id(5)]);
    assert_eq!(errors.len(), 5);
    let affected: HashSet<_> = errors.into_iter().flat_map(|e| e.ids).collect();
    assert_eq!(affected, [id(1), id(2), id(3), id(4), id(9)].into_iter().collect());
    resolve_without_fetch(&service, &context, &id(5));
    service.hydration_requests.write().unwrap().clear();
    *service.hydration_response.write().unwrap() = None;
    assert!(context.ensure_cached(&(1..=5).chain([8, 9]).map(id).collect::<Vec<_>>()).is_empty());
    assert_eq!(
        *service.hydration_requests.read().unwrap(),
        vec![vec![id(1), id(2), id(3), id(4), id(8), id(9)]],
        "invalid results remain misses; the independent success is reused",
    );
}

#[test]
fn blocked_external_id_is_not_conflated_with_a_local_version() {
    let (service, _space, context) = setup();
    let external = HolonId::External(core_types::ExternalId {
        space_id: core_types::OutboundProxyId(LocalId(vec![42])),
        local_id: LocalId(vec![1]),
    });
    let errors = context.ensure_cached(&[external.clone(), id(1), external.clone()]);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].ids, vec![external]);
    assert_eq!(*service.hydration_requests.read().unwrap(), vec![vec![id(1)]]);
    resolve_without_fetch(&service, &context, &id(1));
}

#[test]
fn capacity_pressure_preserves_existing_pins_and_adds_no_leases() {
    let (service, space, _) = setup();
    let context = space
        .get_transaction_manager()
        .open_private_restricted_cache_read_transaction(space.clone())
        .unwrap();
    let mut manager = HolonCacheManager::new(service, Arc::downgrade(&context.space_manager()));
    manager.cache = HolonCache::new_with_capacity(1);
    let lease = manager.retain_holon(&id(1)).unwrap();
    let pinned = manager.cache.get(&id(1)).unwrap();
    assert!(manager.ensure_cached(&[id(1), id(2), id(3)]).is_empty());
    assert!(Arc::ptr_eq(&pinned, &manager.cache.get(&id(1)).unwrap()));
    manager.cache.clear();
    assert!(manager.cache.get(&id(2)).is_none());
    assert!(manager.cache.get(&id(3)).is_none());
    drop(lease);
    assert!(manager.cache.get(&id(1)).is_none());
}

#[test]
fn concurrent_lazy_read_wins_over_in_flight_population() {
    let (service, _space, context) = setup();
    let entered = Arc::new(std::sync::Barrier::new(2));
    let resume = Arc::new(std::sync::Barrier::new(2));
    *service.hydration_barriers.write().unwrap() = Some((entered.clone(), resume.clone()));
    let worker = {
        let context = context.clone();
        std::thread::spawn(move || context.ensure_cached(&[id(1)]))
    };
    entered.wait();
    // Normal reference resolution races with the bulk call, without acquiring its manager lock.
    let reference = HolonReference::smart_from_id(context.space_read_handle(), id(1));
    reference.key().unwrap();
    let concurrent = resolve_without_fetch(&service, &context, &id(1));
    resume.wait();
    assert!(worker.join().unwrap().is_empty());
    let installed = resolve_without_fetch(&service, &context, &id(1));
    assert!(Arc::ptr_eq(&concurrent, &installed));
}

#[test]
fn exact_versions_stay_separate_and_unrelated_staged_edits_are_untouched() {
    use crate::WritableHolon;
    let (service, _space, context) = setup();
    let mut draft = context.mutation().new_holon(Some("node-1".into())).unwrap();
    draft.with_property_value("Description", "uncommitted").unwrap();
    let draft = context.mutation().stage_new_holon(draft).unwrap();
    let transient_count = context.lookup().transient_count().unwrap();
    let version = Holon::Saved(SavedHolon::new(
        LocalId(vec![2]),
        PropertyMap::new(),
        Some(LocalId(vec![1])),
        MapInteger(2),
    ));
    *service.hydration_response.write().unwrap() = Some(Ok(vec![
        SavedHolonFetch { id: id(1), result: Ok(content(1)) },
        SavedHolonFetch { id: id(2), result: Ok(version.clone()) },
    ]));
    assert!(context.ensure_cached(&[id(1), id(2)]).is_empty());
    assert_eq!(*resolve_without_fetch(&service, &context, &id(1)).read().unwrap(), content(1));
    assert_eq!(*resolve_without_fetch(&service, &context, &id(2)).read().unwrap(), version);
    assert_eq!(
        draft.property_value("Description").unwrap(),
        Some(BaseValue::StringValue("uncommitted".into()))
    );
    assert_eq!(context.lookup().staged_count().unwrap(), 1);
    assert_eq!(context.lookup().transient_count().unwrap(), transient_count);
}

#[test]
fn direct_population_uses_its_own_restricted_execution_context() {
    let (service, space, context) = setup();
    service.exercise_context_lifetime.store(true, Ordering::Relaxed);
    let manager = HolonCacheManager::new(service.clone(), Arc::downgrade(&space));
    assert!(manager.ensure_cached(&[]).is_empty());
    assert!(service.read_contexts.read().unwrap().is_empty());
    assert!(manager.ensure_cached(&[id(1)]).is_empty());
    assert!(service.read_contexts.read().unwrap().iter().all(|weak| weak.upgrade().is_none()));
    assert!(context.is_open());
}

#[test]
fn plural_navigation_warms_only_distinct_saved_targets_and_preserves_occurrences() {
    use crate::descriptors::{Descriptor, EffectiveHolonDescriptor};
    let (service, _space, context) = setup();
    let smart = |n| HolonReference::smart_from_id(context.space_read_handle(), id(n));
    let transient = context.mutation().new_holon(None).unwrap();
    let draft = context.mutation().new_holon(Some("draft".into())).unwrap();
    let staged = context.mutation().stage_new_holon(draft).unwrap();
    let members: Vec<HolonReference> =
        vec![smart(1), transient.into(), smart(2), smart(1), staged.into()];
    let mut projection = context.mutation().new_holon(None).unwrap();
    projection.add_related_references("PluralNavigableRelationships", members.clone()).unwrap();
    projection.add_related_references("SingularNavigableRelationships", vec![smart(3)]).unwrap();
    let effective = EffectiveHolonDescriptor::from_constructed_holon(projection.into());
    effective.singular_navigable_relationships().unwrap();
    effective.instance_properties().unwrap();
    assert!(service.hydration_requests.read().unwrap().is_empty());
    let identities = |refs: Vec<crate::descriptors::RelationshipDescriptor>| {
        refs.into_iter().map(|entry| entry.holon().full_reference_id_string()).collect::<Vec<_>>()
    };
    let expected =
        members.iter().map(|member| member.full_reference_id_string()).collect::<Vec<_>>();
    assert_eq!(identities(effective.plural_navigable_relationships().unwrap()), expected);
    assert_eq!(*service.hydration_requests.read().unwrap(), vec![vec![id(1), id(2)]]);
    let reads = service.content_reads.load(Ordering::Relaxed);
    smart(1).key().unwrap();
    smart(2).key().unwrap();
    assert_eq!(service.content_reads.load(Ordering::Relaxed), reads);
    assert_eq!(identities(effective.plural_navigable_relationships().unwrap()), expected);
    assert_eq!(service.hydration_requests.read().unwrap().len(), 1);
    assert_eq!(context.lookup().staged_count().unwrap(), 1);
    assert!(context.is_open());
}

#[test]
fn plural_navigation_warming_failure_preserves_results_and_retries() {
    use crate::descriptors::EffectiveHolonDescriptor;
    let (service, _space, context) = setup();
    let member = HolonReference::smart_from_id(context.space_read_handle(), id(1));
    let mut projection = context.mutation().new_holon(None).unwrap();
    projection
        .add_related_references("PluralNavigableRelationships", vec![member.clone(), member])
        .unwrap();
    let effective = EffectiveHolonDescriptor::from_constructed_holon(projection.into());
    *service.hydration_response.write().unwrap() =
        Some(Err(HolonError::ConductorError("offline".into())));
    assert_eq!(effective.plural_navigable_relationships().unwrap().len(), 2);
    *service.hydration_response.write().unwrap() = None;
    assert_eq!(effective.plural_navigable_relationships().unwrap().len(), 2);
    assert_eq!(*service.hydration_requests.read().unwrap(), vec![vec![id(1)], vec![id(1)]]);
}

#[test]
fn singular_availability_does_not_warm_plural_navigation() {
    use crate::{
        descriptors::{Descriptor, EffectiveHolonDescriptor, PreparedSchema},
        WritableHolon,
    };
    let (service, _space, context) = setup();
    let smart = |n| HolonReference::smart_from_id(context.space_read_handle(), id(n));
    let mut projection = context.mutation().new_holon(None).unwrap();
    projection.add_related_references("DescribedHolonType", vec![smart(2)]).unwrap();
    projection
        .add_related_references("EffectiveInstanceRelationships", vec![smart(5), smart(6)])
        .unwrap();
    projection.add_related_references("SingularNavigableRelationships", vec![smart(5)]).unwrap();
    projection
        .add_related_references("PluralNavigableRelationships", vec![smart(6), smart(7)])
        .unwrap();
    projection
        .with_property_value(
            "PreparedSchema",
            serde_json::to_string(&PreparedSchema::default()).unwrap(),
        )
        .unwrap();
    let effective = EffectiveHolonDescriptor::from_constructed_holon(projection.into());
    let singular = effective.available_singular_relationships(&smart(1)).unwrap();
    assert_eq!(singular.len(), 1);
    assert_eq!(singular[0].holon().holon_id().unwrap(), id(5));
    assert!(service.hydration_requests.read().unwrap().is_empty());
    let plural = effective.available_plural_relationships(&smart(1)).unwrap();
    assert_eq!(plural.len(), 2);
    assert_eq!(*service.hydration_requests.read().unwrap(), vec![vec![id(6), id(7)]]);
}
