use super::*;
use crate::core_shared_objects::{holon::SavedHolon, space_manager::HolonSpaceManager};
use crate::{HolonCollectionApi, ServiceRoutingPolicy, StagedReference, TransientReference};
use crate::{ReadableHolon, RelationshipCachePolicy};
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::{LocalId, PropertyMap, PropertyName};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::{any::Any, collections::HashMap};
use type_names::ToRelationshipName;

#[derive(Debug)]
struct CountingService {
    relationship_barriers: RwLock<Option<(Arc<std::sync::Barrier>, Arc<std::sync::Barrier>)>>,
    hydration_requests: RwLock<Vec<Vec<HolonId>>>,
    hydration_response:
        RwLock<Option<Result<Vec<super::super::cache_population::SavedHolonFetch>, HolonError>>>,
    hydration_barriers: RwLock<Option<(Arc<std::sync::Barrier>, Arc<std::sync::Barrier>)>>,
    read_contexts: RwLock<Vec<Weak<TransactionContext>>>,
    exercise_context_lifetime: std::sync::atomic::AtomicBool,
    fail_reads: std::sync::atomic::AtomicBool,
    preparation_calls: AtomicUsize,
    reuse_frozen: bool,
    reuse_described_by: std::sync::atomic::AtomicBool,
    edges: RwLock<HashMap<(u8, String), Vec<u8>>>,
    calls: RwLock<HashMap<(u8, String), usize>>,
    bulk_calls: AtomicUsize,
    set_calls: AtomicUsize,
    incomplete_set: std::sync::atomic::AtomicBool,
    invalid_bulk: std::sync::atomic::AtomicBool,
    content_reads: AtomicUsize,
    policy_calls: AtomicUsize,
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
            relationship_barriers: Default::default(),
            hydration_requests: RwLock::new(Vec::new()),
            hydration_response: RwLock::new(None),
            hydration_barriers: RwLock::new(None),
            read_contexts: Default::default(),
            exercise_context_lifetime: Default::default(),
            fail_reads: Default::default(),
            preparation_calls: Default::default(),
            reuse_frozen,
            reuse_described_by: Default::default(),
            edges: RwLock::new(edges),
            calls: RwLock::new(HashMap::new()),
            bulk_calls: AtomicUsize::new(0),
            set_calls: AtomicUsize::new(0),
            incomplete_set: std::sync::atomic::AtomicBool::new(false),
            invalid_bulk: Default::default(),
            content_reads: AtomicUsize::new(0),
            policy_calls: AtomicUsize::new(0),
            clock: AtomicU64::new(0),
        }
    }
    fn observe_read_context(&self, context: &Arc<TransactionContext>) -> Result<(), HolonError> {
        if !self.exercise_context_lifetime.load(Ordering::Relaxed) {
            return Ok(());
        }
        assert!(context.is_restricted_cache_read());
        assert!(context
            .space_manager()
            .get_transaction_manager()
            .get_transaction(&context.tx_id())?
            .is_none());
        self.read_contexts.write().unwrap().push(Arc::downgrade(context));
        // Mimic transported transient graphs retaining references to their owner.
        // Dropping the caller's Arc alone cannot reclaim this pool/context cycle.
        let mut envelope = context.mutation().new_holon(None)?;
        envelope.add_related_references("Temporary", vec![envelope.clone().into()])?;
        if self.fail_reads.load(Ordering::Relaxed) {
            return Err(HolonError::ConductorError("fetch failed".into()));
        }
        Ok(())
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
    fn get_saved_holon_by_key_internal(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<crate::SmartReference, HolonError> {
        use type_names::CoreHolonTypeName;
        self.observe_read_context(context)?;
        if *key
            == CoreHolonTypeName::EffectiveHolonDescriptorRegistry
                .descriptor_key(&CoreHolonTypeName::HolonType)
        {
            return Ok(crate::SmartReference::new_from_id(
                context.space_read_handle(),
                HolonId::Local(LocalId(vec![13])),
            ));
        }
        Err(HolonError::NotImplemented("unexpected saved-key lookup".into()))
    }
    fn prepare_relationship_policy_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source: &HolonId,
        _name: &RelationshipName,
    ) -> Result<(), HolonError> {
        self.preparation_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
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
    ) -> Result<Option<RelationshipCachePolicy>, HolonError> {
        self.policy_calls.fetch_add(1, Ordering::Relaxed);
        Ok(Some(
            if (self.reuse_frozen && name.to_string().starts_with("Frozen"))
                || (self.reuse_described_by.load(Ordering::Relaxed)
                    && name.to_string() == "DescribedBy")
            {
                RelationshipCachePolicy::Reuse
            } else if name.to_string() == "Bounded" {
                RelationshipCachePolicy::MaxAgeMillis(10)
            } else {
                RelationshipCachePolicy::Fresh
            },
        ))
    }
    fn fetch_relationship_set_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        names: &[RelationshipName],
    ) -> Result<Vec<(RelationshipName, HolonCollection)>, HolonError> {
        self.set_calls.fetch_add(1, Ordering::Relaxed);
        let mut result = names
            .iter()
            .map(|name| {
                Ok((name.clone(), self.fetch_related_holons_internal(context, source, name)?))
            })
            .collect::<Result<Vec<_>, HolonError>>()?;
        if self.incomplete_set.load(Ordering::Relaxed) {
            result.pop();
        }
        Ok(result)
    }
    fn fetch_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        ids: &[HolonId],
    ) -> Result<Vec<super::super::cache_population::SavedHolonFetch>, HolonError> {
        self.observe_read_context(context)?;
        assert!(context.is_restricted_cache_read());
        self.hydration_requests.write().unwrap().push(ids.to_vec());
        let barriers = self.hydration_barriers.read().unwrap().clone();
        if let Some((entered, resume)) = barriers {
            entered.wait();
            resume.wait();
        }
        if let Some(response) = self.hydration_response.read().unwrap().clone() {
            return response;
        }
        Ok(ids
            .iter()
            .rev()
            .map(|id| super::super::cache_population::SavedHolonFetch {
                id: id.clone(),
                result: self.fetch_holon_internal(context, id),
            })
            .collect())
    }
    fn fetch_holon_internal(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        self.observe_read_context(context)?;
        self.content_reads.fetch_add(1, Ordering::Relaxed);
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
        self.observe_read_context(context)?;
        let key = (id.local_id().0[0], name.to_string());
        *self.calls.write().unwrap().entry(key.clone()).or_default() += 1;
        let targets = self.edges.read().unwrap().get(&key).cloned().unwrap_or_default();
        let barriers = self.relationship_barriers.write().unwrap().take();
        if let Some((entered, resume)) = barriers {
            entered.wait();
            resume.wait();
        }
        // The cache must seal even a mutable collection supplied by the service.
        let mut collection = HolonCollection::new_transient();
        collection
            .add_references(targets.into_iter().map(|id| Self::reference(context, id)).collect())?;
        Ok(collection)
    }
    fn fetch_all_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        self.observe_read_context(context)?;
        self.bulk_calls.fetch_add(1, Ordering::Relaxed);
        if self.fail_reads.load(Ordering::Relaxed) {
            return Err(HolonError::ConductorError("bulk fetch failed".into()));
        }
        let mut result = RelationshipMap::new_empty();
        for ((id, name), targets) in self.edges.read().unwrap().iter() {
            if *id != source.local_id().0[0] {
                continue;
            }
            let mut collection = HolonCollection::new_transient();
            collection
                .add_references(targets.iter().map(|id| Self::reference(context, *id)).collect())?;
            if self.invalid_bulk.load(Ordering::Relaxed) && name == "Moving" {
                collection.add_references(vec![context.mutation().new_holon(None)?.into()])?;
            }
            result.insert(name.as_str().to_relationship_name(), Arc::new(RwLock::new(collection)));
        }
        Ok(result)
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
fn smart_reference_access_checks_do_not_fetch_saved_content() -> Result<(), HolonError> {
    use crate::core_shared_objects::holon::state::AccessType;

    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    // Exercise both bare saved identities and references carrying smart properties.
    let references = [
        HolonReference::smart_from_id(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![1])),
        ),
        CountingService::reference(&context, 1),
    ];
    for reference in references {
        for access in [AccessType::Read, AccessType::Clone] {
            reference.is_accessible(access)?;
            assert_eq!(service.content_reads.load(Ordering::Relaxed), 0);
        }
        for access in [AccessType::Write, AccessType::Commit, AccessType::Abandon] {
            let expected_access = access.to_string();
            assert!(matches!(
                reference.is_accessible(access),
                Err(HolonError::NotAccessible(operation, state))
                    if operation == expected_access && state == "Saved"
            ));
            assert_eq!(service.content_reads.load(Ordering::Relaxed), 0);
        }
    }
    Ok(())
}

#[test]
fn smart_property_hits_avoid_fetching_and_misses_resolve_saved_content() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let reference = CountingService::reference(&context, 1);
    assert_eq!(reference.property_value("Key")?, Some(BaseValue::StringValue("node-1".into())));
    assert_eq!(service.content_reads.load(Ordering::Relaxed), 0);

    assert_eq!(reference.property_value("TypeName")?, Some(BaseValue::StringValue("Type".into())));
    assert_eq!(service.content_reads.load(Ordering::Relaxed), 1);
    assert_eq!(reference.property_value("Missing")?, None);
    assert_eq!(service.content_reads.load(Ordering::Relaxed), 1);
    Ok(())
}

#[test]
fn all_relationship_reads_preserve_declared_content_outside_prepared_navigation(
) -> Result<(), HolonError> {
    use crate::core_shared_objects::descriptor_registry::EffectiveHolonDescriptorRegistry;
    use crate::descriptors::{EffectiveHolonDescriptor, PreparedSchema};
    use crate::WritableHolon;

    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service,
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let registry = Arc::new(EffectiveHolonDescriptorRegistry::new(&space));
    space.attach_descriptor_registry(&registry);
    let owner = registry.context()?;
    let reference = |id| CountingService::reference(&owner, id);
    let mut prepared = owner.mutation().new_holon(None)?;
    prepared.add_related_references("DescribedHolonType", vec![reference(2)])?;
    prepared.add_related_references(
        "EffectiveInstanceRelationships",
        vec![reference(5), reference(6), reference(12)],
    )?;
    // Presentation navigation is deliberately narrower than the authored contract.
    prepared.add_related_references("SingularNavigableRelationships", vec![reference(6)])?;
    prepared.with_property_value(
        "PreparedSchema",
        serde_json::to_string(&PreparedSchema {
            relationships: [
                (5, "Frozen", RelationshipCachePolicy::Reuse),
                (6, "Moving", RelationshipCachePolicy::Fresh),
                (12, "DescribedBy", RelationshipCachePolicy::Reuse),
            ]
            .into_iter()
            .map(|(id, name, policy)| crate::descriptors::PreparedRelationship {
                identity: reference(id).full_reference_id_string(),
                name: name.to_relationship_name(),
                policy,
                updateable: true,
            })
            .collect(),
            ..PreparedSchema::default()
        })
        .unwrap(),
    )?;
    registry.publish(
        HolonId::Local(LocalId(vec![2])),
        &EffectiveHolonDescriptor::from_constructed_holon(prepared.into()),
    )?;
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let source = CountingService::reference(&context, 1);
    let available =
        crate::reference_layer::available_relationships::available_relationships(&source)?;
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].descriptor.base_relationship_name()?.to_string(), "Moving");

    let all = source.all_related_holons()?;
    let frozen = all
        .get_collection_for_relationship(&"Frozen".to_relationship_name())
        .expect("all reads must retain declared content omitted from presentation navigation");
    assert_eq!(
        frozen.read().unwrap().get_members(),
        &vec![CountingService::reference(&context, 10)]
    );
    Ok(())
}

/// A saved prepared projection whose policy differs deliberately from the fake
/// provider for Empty/Incoming, so accidentally falling back changes observable reuse.
fn prepared_cache_fixture() -> Result<
    (
        Arc<CountingService>,
        Arc<HolonSpaceManager>,
        Arc<crate::core_shared_objects::descriptor_registry::EffectiveHolonDescriptorRegistry>,
        HolonCacheManager,
    ),
    HolonError,
> {
    use crate::core_shared_objects::descriptor_registry::EffectiveHolonDescriptorRegistry;
    use crate::descriptors::{EffectiveHolonDescriptor, PreparedRelationship, PreparedSchema};
    use crate::WritableHolon;
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let registry = Arc::new(EffectiveHolonDescriptorRegistry::new(&space));
    space.attach_descriptor_registry(&registry);
    let owner = registry.context()?;
    let reference = |id| CountingService::reference(&owner, id);
    service.reuse_described_by.store(true, Ordering::Relaxed);
    let definitions = [
        (5, "Frozen", RelationshipCachePolicy::Reuse, true),
        (6, "Moving", RelationshipCachePolicy::Fresh, true),
        (7, "Incoming", RelationshipCachePolicy::Reuse, false),
        (12, "DescribedBy", RelationshipCachePolicy::Reuse, true),
        (13, "Empty", RelationshipCachePolicy::Reuse, true),
        (14, "Bounded", RelationshipCachePolicy::MaxAgeMillis(10), true),
        (15, "ExcludedInverse", RelationshipCachePolicy::Reuse, false),
        (16, "Extends", RelationshipCachePolicy::Reuse, true),
    ];
    let mut result = owner.mutation().new_holon(None)?;
    result.add_related_references("DescribedHolonType", vec![reference(2)])?;
    result.add_related_references(
        "EffectiveInstanceRelationships",
        vec![reference(5), reference(6), reference(12), reference(13), reference(14)],
    )?;
    // Frozen is declared but omitted from presentation; ExcludedInverse represents
    // maximum-zero navigation removed by the constructor's partitioning. Extends
    // is navigable but not declared in this type's instance contract.
    result.add_related_references(
        "PluralNavigableRelationships",
        vec![reference(6), reference(7), reference(16)],
    )?;
    result.with_property_value(
        "PreparedSchema",
        serde_json::to_string(&PreparedSchema {
            relationships: definitions
                .into_iter()
                .map(|(id, name, policy, updateable)| PreparedRelationship {
                    identity: reference(id).full_reference_id_string(),
                    name: name.to_relationship_name(),
                    policy,
                    updateable,
                })
                .collect(),
            navigation_members: vec![
                reference(6).holon_id()?,
                reference(7).holon_id()?,
                reference(15).holon_id()?,
                reference(16).holon_id()?,
            ],
            ..PreparedSchema::default()
        })
        .unwrap(),
    )?;
    registry.publish(
        reference(2).holon_id()?,
        &EffectiveHolonDescriptor::from_constructed_holon(result.into()),
    )?;
    let manager = HolonCacheManager::new(service.clone(), Arc::downgrade(&space));
    // Model the foundational DescribedBy association already resident on a host.
    let cache = manager.relationship_cache.read().unwrap().clone();
    cache.related_holons(
        &manager.transactions,
        service.as_ref(),
        &reference(1).holon_id()?,
        &"DescribedBy".to_relationship_name(),
        |_| Ok(Some(RelationshipCachePolicy::Reuse)),
    )?;
    reference(1).holon_descriptor()?;
    service.content_reads.store(0, Ordering::Relaxed);
    service.hydration_requests.write().unwrap().clear();
    Ok((service, space, registry, manager))
}

#[test]
fn prepared_bindings_avoid_schema_reads_and_preserve_membership_freshness() -> Result<(), HolonError>
{
    let (service, space, _registry, manager) = prepared_cache_fixture()?;
    let source = HolonId::Local(LocalId(vec![1]));
    let before_policy = service.policy_calls.load(Ordering::Relaxed);
    let before_preparation = service.preparation_calls.load(Ordering::Relaxed);
    let mut memberships = HashMap::new();
    for name in ["Frozen", "Moving", "Empty", "Incoming", "Bounded"] {
        let first = manager.get_related_holons(&source, &name.to_relationship_name())?;
        let second = manager.get_related_holons(&source, &name.to_relationship_name())?;
        assert_eq!(Arc::ptr_eq(&first, &second), name != "Moving");
        memberships.insert(name, second);
    }
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), before_policy);
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), before_preparation);
    assert_eq!(service.content_reads.load(Ordering::Relaxed), 0);
    assert!(service.hydration_requests.read().unwrap().is_empty());
    let fresh = manager.get_related_holons_with_hint(
        &source,
        &"Empty".to_relationship_name(),
        crate::RelationshipReadHint::RequireFresh,
    )?;
    assert!(!Arc::ptr_eq(&fresh, &memberships["Empty"]));
    service.clock.store(10, Ordering::Relaxed);
    let expired = manager.get_related_holons(&source, &"Bounded".to_relationship_name())?;
    assert!(!Arc::ptr_eq(&expired, &memberships["Bounded"]));
    assert_eq!(service.count("Bounded"), 2);
    space.reset_session()?;
    assert!(Arc::ptr_eq(
        &fresh,
        &manager.get_related_holons(&source, &"Empty".to_relationship_name())?
    ));
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), before_preparation);
    // No current preparation: an unresolved name still reaches normal provider resolution.
    manager.get_related_holons(&source, &"Unprepared".to_relationship_name())?;
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), before_preparation + 1);
    Ok(())
}

#[test]
fn prepared_bulk_vocabulary_uses_declared_and_filtered_navigation_without_warming(
) -> Result<(), HolonError> {
    let (service, _space, _registry, manager) = prepared_cache_fixture()?;
    let source = HolonId::Local(LocalId(vec![1]));
    let before_policy = service.policy_calls.load(Ordering::Relaxed);
    let before_preparation = service.preparation_calls.load(Ordering::Relaxed);
    let all = manager.get_all_related_holons(&source)?;
    let names: std::collections::HashSet<_> =
        all.iter().into_iter().map(|(name, _)| name.to_string()).collect();
    assert_eq!(
        names,
        ["Frozen", "Moving", "Incoming", "DescribedBy", "Empty", "Bounded", "Extends"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 1);
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), before_policy);
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), before_preparation);
    assert_eq!(service.content_reads.load(Ordering::Relaxed), 0);
    assert!(service.hydration_requests.read().unwrap().is_empty());
    let empty = all.get_collection_for_relationship(&"Empty".to_relationship_name()).unwrap();
    assert_eq!(empty.read().unwrap().get_members().len(), 0);
    assert!(Arc::ptr_eq(
        &empty,
        &manager.get_related_holons(&source, &"Empty".to_relationship_name())?
    ));
    service.clock.store(10, Ordering::Relaxed);
    manager.get_all_related_holons(&source)?;
    assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 2);
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), before_preparation);
    Ok(())
}

#[test]
fn prepared_membership_batch_reuses_policies_without_warming_or_changing_freshness(
) -> Result<(), HolonError> {
    let (service, _space, _registry, manager) = prepared_cache_fixture()?;
    let id = HolonId::Local(LocalId(vec![1]));
    let names: Vec<_> = ["Empty", "Moving", "Incoming"]
        .into_iter()
        .map(|name| name.to_relationship_name())
        .collect();
    let before = service.preparation_calls.load(Ordering::Relaxed);
    manager.prepare_relationships(&id, &names)?;
    manager.prepare_relationships(&id, &names)?;
    assert_eq!(service.count("Empty"), 1);
    assert_eq!(service.count("Incoming"), 1);
    assert_eq!(service.count("Moving"), 2);
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), before);
    assert!(service.hydration_requests.read().unwrap().is_empty());
    Ok(())
}

#[test]
fn descriptor_sources_keep_guest_binding_of_their_own_ancestry() -> Result<(), HolonError> {
    use crate::descriptors::{EffectiveHolonDescriptor, PreparedRelationship, PreparedSchema};
    use crate::WritableHolon;
    let (service, _space, registry, manager) = prepared_cache_fixture()?;
    let owner = registry.context()?;
    let reference = |id| CountingService::reference(&owner, id);
    let mut result = owner.mutation().new_holon(None)?;
    result.add_related_references("DescribedHolonType", vec![reference(2)])?;
    result.add_related_references(
        "EffectiveInstanceRelationships",
        vec![reference(5), reference(16)],
    )?;
    result.with_property_value(
        "PreparedSchema",
        serde_json::to_string(&PreparedSchema {
            relationships: [(5, "Frozen"), (16, "Extends")]
                .into_iter()
                .map(|(id, name)| PreparedRelationship {
                    identity: reference(id).full_reference_id_string(),
                    name: name.to_relationship_name(),
                    policy: RelationshipCachePolicy::Reuse,
                    updateable: true,
                })
                .collect(),
            ..PreparedSchema::default()
        })
        .unwrap(),
    )?;
    registry.publish(
        reference(2).holon_id()?,
        &EffectiveHolonDescriptor::from_constructed_holon(result.into()),
    )?;
    let before = service.preparation_calls.load(Ordering::Relaxed);
    manager.get_related_holons(&reference(1).holon_id()?, &"Frozen".to_relationship_name())?;
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), before + 1);
    manager.get_all_related_holons(&reference(1).holon_id()?)?;
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), before + 2);
    Ok(())
}

#[test]
fn prepared_binding_preserves_exact_versions_and_rejects_ambiguous_names() -> Result<(), HolonError>
{
    use crate::descriptors::{EffectiveHolonDescriptor, PreparedRelationship, PreparedSchema};
    use crate::WritableHolon;
    let (service, space, registry, _manager) = prepared_cache_fixture()?;
    let owner = registry.context()?;
    let first = HolonReference::smart_from_id(
        owner.space_read_handle(),
        HolonId::Local(LocalId(vec![5, 1])),
    );
    let second = HolonReference::smart_from_id(
        owner.space_read_handle(),
        HolonId::Local(LocalId(vec![5, 2])),
    );
    let mut result = owner.mutation().new_holon(None)?;
    result.add_related_references(
        "DescribedHolonType",
        vec![CountingService::reference(&owner, 2)],
    )?;
    result.add_related_references(
        "EffectiveInstanceRelationships",
        vec![first.clone(), second.clone()],
    )?;
    let mut metadata = PreparedSchema {
        relationships: [(&first, "First"), (&second, "Second")]
            .into_iter()
            .map(|(reference, name)| PreparedRelationship {
                identity: reference.full_reference_id_string(),
                name: name.to_relationship_name(),
                policy: RelationshipCachePolicy::Reuse,
                updateable: true,
            })
            .collect(),
        ..PreparedSchema::default()
    };
    result.with_property_value("PreparedSchema", serde_json::to_string(&metadata).unwrap())?;
    let effective = EffectiveHolonDescriptor::from_constructed_holon(result.clone().into());
    assert_eq!(
        effective
            .saved_relationship_binding(&"First".to_relationship_name())?
            .unwrap()
            .descriptor_id,
        first.holon_id()?
    );
    assert_eq!(
        effective
            .saved_relationship_binding(&"Second".to_relationship_name())?
            .unwrap()
            .descriptor_id,
        second.holon_id()?
    );
    metadata.relationships[1].name = "First".to_relationship_name();
    result.with_property_value("PreparedSchema", serde_json::to_string(&metadata).unwrap())?;
    assert!(matches!(
        effective.saved_relationship_binding(&"First".to_relationship_name()),
        Err(HolonError::AmbiguousRelationshipTraversal { .. })
    ));
    let retained = registry.get(&HolonId::Local(LocalId(vec![2])))?.unwrap();
    space.reset_session()?;
    assert_eq!(retained.saved_content_relationship_bindings(), Err(HolonError::ObsoleteSession));
    assert_eq!(service.content_reads.load(Ordering::Relaxed), 0);
    Ok(())
}

#[test]
fn named_and_all_reads_share_definitional_cache_and_keep_mutable_membership_fresh(
) -> Result<(), HolonError> {
    {
        for named_first in [true, false] {
            let service = Arc::new(CountingService::new(true));
            let space = HolonSpaceManager::new_with_managers(
                None,
                service.clone(),
                None,
                ServiceRoutingPolicy::BlockExternal,
            );
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
            assert_eq!(service.count("Frozen"), usize::from(named_first));
            assert_eq!(service.count("Moving"), 0);
            assert_eq!(service.count("Incoming"), 0);
            assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 1);
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
            assert_eq!(service.count("Frozen"), usize::from(named_first));
            for name in ["Moving", "Incoming"] {
                let collection =
                    all.get_collection_for_relationship(&name.to_relationship_name()).unwrap();
                assert_eq!(
                    collection.read().unwrap().get_members(),
                    &vec![CountingService::reference(&next, 11)]
                );
                assert_eq!(service.count(name), 0);
                let named = source.related_holons(name)?;
                assert_eq!(
                    named.read().unwrap().get_members(),
                    collection.read().unwrap().get_members()
                );
                assert!(!Arc::ptr_eq(&named, &collection));
                assert_eq!(service.count(name), 1);
            }
            assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 2);
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
            let space = HolonSpaceManager::new_with_managers(
                None,
                service.clone(),
                None,
                ServiceRoutingPolicy::BlockExternal,
            );
            let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
            let manager =
                HolonCacheManager::new(service.clone(), Arc::downgrade(&context.space_manager()));
            let id = HolonId::Local(LocalId(vec![1]));
            let name = "Frozen".to_relationship_name();
            let probe = crate::core_shared_objects::read_observations::CacheReadProbe::start(true);
            let first = manager.get_related_holons(&id, &name)?;
            let before = service.calls.read().unwrap().clone();
            let again = manager.get_related_holons(&id, &name)?;
            assert!(Arc::ptr_eq(&first, &again));
            let cache = manager.relationship_cache.read().unwrap().clone();
            let hit = cache.related_holons(
                &CacheReadTransactions { space: Arc::downgrade(&context.space_manager()) },
                service.as_ref(),
                &id,
                &name,
                |_| panic!("cache hits must not resolve descriptors"),
            )?;
            assert!(Arc::ptr_eq(&first, &hit));
            assert_eq!(*service.calls.read().unwrap(), before);
            let observations = probe.finish();
            let requests = |outcome| {
                observations
                    .iter()
                    .filter(|read| {
                        read.source == id
                            && read.relationship.as_ref() == Some(&name)
                            && read.outcome == outcome
                    })
                    .map(|read| read.requests)
                    .sum::<usize>()
            };
            assert_eq!(requests("missing"), 1);
            assert_eq!(requests("hit"), 2, "known-empty sets also produce cache hits");
            let moving = manager.get_related_holons(&id, &"Moving".to_relationship_name())?;
            let again = manager.get_related_holons(&id, &"Moving".to_relationship_name())?;
            assert!(!Arc::ptr_eq(&moving, &again));
        }
    }
    Ok(())
}

#[test]
fn miss_fetches_before_classification_and_failed_classification_is_not_cached(
) -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(false));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = RelationshipCache::new();
    let id = HolonId::Local(LocalId(vec![1]));
    let name = "Frozen".to_relationship_name();
    let result = cache.related_holons(
        &CacheReadTransactions { space: Arc::downgrade(&context.space_manager()) },
        service.as_ref(),
        &id,
        &name,
        |_| {
            assert_eq!(service.count("Frozen"), 1);
            Err(HolonError::InvalidState("classification failed".into()))
        },
    );
    assert!(matches!(result, Err(HolonError::InvalidState(_))));
    cache.related_holons(
        &CacheReadTransactions { space: Arc::downgrade(&context.space_manager()) },
        service.as_ref(),
        &id,
        &name,
        |_| {
            assert_eq!(service.count("Frozen"), 2);
            Ok(Some(RelationshipCachePolicy::Reuse))
        },
    )?;
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
    ) -> Result<Option<RelationshipCachePolicy>, HolonError> {
        Ok(Some(RelationshipCachePolicy::Reuse))
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
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
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
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = RelationshipCache::new();
    let id = HolonId::Local(LocalId(vec![1]));
    let name = "Moving".to_relationship_name();
    service.edges.write().unwrap().insert((1, "Moving".into()), vec![]);
    let policy_calls = AtomicUsize::new(0);
    let read = |hint| {
        cache.related_holons_with_hint(
            &CacheReadTransactions { space: Arc::downgrade(&context.space_manager()) },
            service.as_ref(),
            &id,
            &name,
            hint,
            |_| {
                policy_calls.fetch_add(1, Ordering::Relaxed);
                Ok(Some(RelationshipCachePolicy::MaxAgeMillis(30_000)))
            },
        )
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
    assert_eq!(policy_calls.load(Ordering::Relaxed), 1);
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
        let space = HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        );
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
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
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
        let space = HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        );
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
        let next_request = HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        );
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

#[test]
fn requested_memberships_reuse_empty_results_without_unrelated_reads_or_hydration(
) -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(true));
    let manager = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = manager.get_transaction_manager().open_public_transaction(manager.clone())?;
    let reference = CountingService::reference(&context, 1);
    let HolonReference::Smart(saved) = &reference else { unreachable!() };
    let names = vec!["Frozen".to_relationship_name(), "FrozenEmpty".to_relationship_name()];
    saved.prepare_relationships(&names)?;
    saved.prepare_relationships(&names)?;
    assert_eq!(service.set_calls.load(Ordering::Relaxed), 1);
    assert_eq!(service.content_reads.load(Ordering::Relaxed), 0);
    assert!(reference.related_holons("FrozenEmpty")?.read().unwrap().get_members().is_empty());
    assert_eq!(service.count("FrozenEmpty"), 1);
    assert_eq!(service.count("Moving"), 0);
    Ok(())
}

#[test]
fn incomplete_requested_set_does_not_install_partial_memberships() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(true));
    let manager = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = manager.get_transaction_manager().open_public_transaction(manager.clone())?;
    let HolonReference::Smart(saved) = CountingService::reference(&context, 1) else {
        unreachable!()
    };
    let names = vec!["Frozen".to_relationship_name(), "FrozenEmpty".to_relationship_name()];
    service.incomplete_set.store(true, Ordering::Relaxed);
    assert!(saved.prepare_relationships(&names).is_err());
    service.incomplete_set.store(false, Ordering::Relaxed);
    saved.prepare_relationships(&names)?;
    assert_eq!(service.count("Frozen"), 2);
    assert_eq!(service.count("FrozenEmpty"), 2);
    Ok(())
}

#[test]
fn retained_content_survives_eviction_pressure_until_last_lease_drops() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let mut manager =
        HolonCacheManager::new(service.clone(), Arc::downgrade(&context.space_manager()));
    manager.cache = HolonCache::new_with_capacity(1);
    let id = HolonId::Local(LocalId(vec![1]));
    let first = manager.retain_holon(&id)?;
    let second = manager.retain_holon(&id)?;
    manager.cache.clear();
    drop(first);
    assert!(manager.cache.get(&id).is_some());
    drop(second);
    assert!(manager.cache.get(&id).is_none());
    Ok(())
}

#[test]
fn fresh_membership_retains_its_policy_without_retaining_members() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(false));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = RelationshipCache::new();
    let id = HolonId::Local(LocalId(vec![1]));
    let name = "Moving".to_relationship_name();
    let policy_calls = AtomicUsize::new(0);
    for _ in 0..3 {
        cache.related_holons(
            &CacheReadTransactions { space: Arc::downgrade(&context.space_manager()) },
            service.as_ref(),
            &id,
            &name,
            |_| {
                policy_calls.fetch_add(1, Ordering::Relaxed);
                Ok(Some(RelationshipCachePolicy::Fresh))
            },
        )?;
    }
    assert_eq!(service.count("Moving"), 3);
    assert_eq!(policy_calls.load(Ordering::Relaxed), 1);
    Ok(())
}

#[test]
fn unresolved_policy_is_retried_until_authoritative_fresh_is_retained() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(false));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let cache = RelationshipCache::new();
    let id = HolonId::Local(LocalId(vec![1]));
    let name = "Moving".to_relationship_name();
    cache.related_holons(
        &CacheReadTransactions { space: Arc::downgrade(&context.space_manager()) },
        service.as_ref(),
        &id,
        &name,
        |_| Ok(None),
    )?;
    cache.related_holons(
        &CacheReadTransactions { space: Arc::downgrade(&context.space_manager()) },
        service.as_ref(),
        &id,
        &name,
        |_| Ok(Some(RelationshipCachePolicy::Fresh)),
    )?;
    cache.related_holons(
        &CacheReadTransactions { space: Arc::downgrade(&context.space_manager()) },
        service.as_ref(),
        &id,
        &name,
        |_| panic!("resolved Fresh must survive membership refresh"),
    )?;
    assert_eq!(service.count("Moving"), 3);
    Ok(())
}

#[test]
fn batch_and_named_reads_share_policies_across_expiration_and_public_transactions(
) -> Result<(), HolonError> {
    use crate::RelationshipReadHint;
    let service = Arc::new(CountingService::new(false));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let first_context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let first = CountingService::reference(&first_context, 1);
    let HolonReference::Smart(saved) = &first else { unreachable!() };
    let names = vec!["Moving".to_relationship_name(), "Bounded".to_relationship_name()];
    saved.prepare_relationships(&names)?;
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), 2);
    first.related_holons("Moving")?;
    first.related_holons_with_hint("Bounded", RelationshipReadHint::RequireFresh)?;
    service.clock.store(10, Ordering::Relaxed);
    let next_context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let HolonReference::Smart(next) = CountingService::reference(&next_context, 1) else {
        unreachable!()
    };
    next.prepare_relationships(&names)?;
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), 2);
    assert_eq!(service.count("Moving"), 3);
    assert_eq!(service.count("Bounded"), 3);
    assert_eq!(service.content_reads.load(Ordering::Relaxed), 0);
    Ok(())
}

mod population;

#[test]
fn private_fetch_contexts_are_reclaimed_on_success_and_failure() -> Result<(), HolonError> {
    for fail in [false, true] {
        let service = Arc::new(CountingService::new(true));
        service.exercise_context_lifetime.store(true, Ordering::Relaxed);
        service.fail_reads.store(fail, Ordering::Relaxed);
        let space = HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        );
        let manager = HolonCacheManager::new(service.clone(), Arc::downgrade(&space));
        let id = HolonId::Local(LocalId(vec![1]));
        for _ in 0..4 {
            assert_eq!(manager.get_rc_holon(&id).is_err(), fail);
            assert_eq!(
                manager.get_related_holons(&id, &"Frozen".to_relationship_name()).is_err(),
                fail
            );
            assert_eq!(
                manager.prepare_relationships(&id, &["Frozen".to_relationship_name()]).is_err(),
                fail
            );
            assert_eq!(
                !manager.ensure_cached(&[HolonId::Local(LocalId(vec![10]))]).is_empty(),
                fail
            );
            let contexts = service.read_contexts.read().unwrap();
            assert!(!contexts.is_empty());
            assert!(
                contexts.iter().all(|context| context.upgrade().is_none()),
                "private context survived completed fetch"
            );
        }
        // Successful saved content remains usable after the fetch owner is gone.
        if !fail {
            assert!(manager
                .get_rc_holon(&id)?
                .read()
                .unwrap()
                .property_map_clone()
                .contains_key(&PropertyName("TypeName".into())));
        }
    }
    Ok(())
}

#[test]
fn saved_cache_hits_create_no_transactions_or_policy_preparation() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let public = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let saved = crate::SmartReference::new_from_id(
        public.space_read_handle(),
        HolonId::Local(LocalId(vec![1])),
    );
    saved.property_value("TypeName")?;
    saved.prepare_relationships(&["Frozen".to_relationship_name()])?;
    let before = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let preparations = service.preparation_calls.load(Ordering::Relaxed);
    let reads = service.content_reads.load(Ordering::Relaxed);
    for _ in 0..8 {
        saved.property_value("TypeName")?;
        saved.related_holons("Frozen")?;
        saved.prepare_relationships(&["Frozen".to_relationship_name()])?;
        let _retained = saved.retain_content()?;
        assert!(public.ensure_cached(&[saved.holon_id()]).is_empty());
    }
    let after = space.get_transaction_manager().open_public_transaction(space.clone())?;
    assert_eq!(after.tx_id().value(), before.tx_id().value() + 1);
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), preparations);
    assert_eq!(service.content_reads.load(Ordering::Relaxed), reads);
    assert!(public.is_open());
    Ok(())
}

#[test]
fn canonical_saved_key_lookup_disposes_temporary_execution_state() -> Result<(), HolonError> {
    use crate::descriptors::resolve_core_descriptor_for;
    use type_names::CoreHolonTypeName;
    for fail in [false, true] {
        let service = Arc::new(CountingService::new(true));
        service.exercise_context_lifetime.store(true, Ordering::Relaxed);
        service.fail_reads.store(fail, Ordering::Relaxed);
        let space = HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        );
        let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let source = CountingService::reference(&context, 1);
        let key = CoreHolonTypeName::EffectiveHolonDescriptorRegistry
            .descriptor_key(&CoreHolonTypeName::HolonType);
        let result = resolve_core_descriptor_for(&source, &key.0);
        assert_eq!(result.is_err(), fail);
        assert!(service.read_contexts.read().unwrap().iter().all(|weak| weak.upgrade().is_none()));
        if let Ok(saved) = result {
            assert_eq!(saved.holon_id()?, HolonId::Local(LocalId(vec![13])));
        }
        assert!(context.is_open());
    }
    Ok(())
}

#[test]
fn derived_reset_preserves_reuse_membership_saved_content_and_staged_work() -> Result<(), HolonError>
{
    use crate::WritableHolon;
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let reference = CountingService::reference(&context, 1);
    reference.property_value("Key")?;
    let node_fetches = service.content_reads.load(Ordering::Relaxed);
    let old = reference.related_holons("Frozen")?;
    service.edges.write().unwrap().insert((1, "Frozen".into()), vec![11]);
    assert!(Arc::ptr_eq(&old, &reference.related_holons("Frozen")?));
    let mut transient = context.mutation().new_holon(Some("session-staged".into()))?;
    transient.with_property_value("Title", "preserved")?;
    let staged = context.mutation().stage_new_holon(transient)?;
    let staged_before = context.lookup().staged_count()?;
    space.reset_session()?;
    assert!(context.is_open());
    assert_eq!(context.lookup().staged_count()?, staged_before);
    assert_eq!(staged.property_value("Title")?, Some(BaseValue::StringValue("preserved".into())));
    reference.property_value("Key")?;
    assert_eq!(service.content_reads.load(Ordering::Relaxed), node_fetches);
    let new = reference.related_holons("Frozen")?;
    assert!(Arc::ptr_eq(&old, &new));
    assert_eq!(new.read().unwrap().get_members(), &vec![CountingService::reference(&context, 10)]);
    // A fresh caller hint, rather than derived retirement, overrides membership reuse.
    let fresh =
        reference.related_holons_with_hint("Frozen", crate::RelationshipReadHint::RequireFresh)?;
    assert_eq!(
        fresh.read().unwrap().get_members(),
        &vec![CountingService::reference(&context, 11)]
    );
    Ok(())
}

#[test]
fn in_flight_relationship_fetch_keeps_its_policy_across_derived_reset() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let reference = CountingService::reference(&context, 1);
    let entered = Arc::new(std::sync::Barrier::new(2));
    let resume = Arc::new(std::sync::Barrier::new(2));
    *service.relationship_barriers.write().unwrap() = Some((entered.clone(), resume.clone()));
    let worker_reference = reference.clone();
    let worker = std::thread::spawn(move || worker_reference.related_holons("Frozen"));
    entered.wait();
    space.reset_session()?;
    service.edges.write().unwrap().insert((1, "Frozen".into()), vec![11]);
    resume.wait();
    let fetched = worker.join().unwrap()?;
    let fresh = reference.related_holons("Frozen")?;
    assert!(Arc::ptr_eq(&fetched, &fresh));
    assert_eq!(
        fresh.read().unwrap().get_members(),
        &vec![CountingService::reference(&context, 10)]
    );
    assert_eq!(service.calls.read().unwrap().get(&(1, "Frozen".into())), Some(&1));
    Ok(())
}

#[test]
fn derived_reset_does_not_override_fresh_or_bounded_membership_policies() -> Result<(), HolonError>
{
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let reference = CountingService::reference(&context, 1);
    for name in ["Moving", "Bounded"] {
        service.edges.write().unwrap().insert((1, name.into()), vec![10]);
        let before = reference.related_holons(name)?;
        service.edges.write().unwrap().insert((1, name.into()), vec![11]);
        space.reset_session()?;
        let after = reference.related_holons(name)?;
        if name == "Bounded" {
            assert!(Arc::ptr_eq(&before, &after));
            service.clock.store(11, Ordering::Relaxed);
            let expired = reference.related_holons(name)?;
            assert_eq!(
                expired.read().unwrap().get_members(),
                &vec![CountingService::reference(&context, 11)]
            );
        } else {
            assert!(!Arc::ptr_eq(&before, &after));
            assert_eq!(
                after.read().unwrap().get_members(),
                &vec![CountingService::reference(&context, 11)]
            );
        }
    }
    Ok(())
}

#[test]
fn bulk_cache_coverage_includes_known_empty_and_obeys_expiry() -> Result<(), HolonError> {
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let transactions = CacheReadTransactions { space: Arc::downgrade(&space) };
    let cache = crate::core_shared_objects::relationship_cache::RelationshipCache::new();
    let id = HolonId::Local(LocalId(vec![1]));
    let names = vec![
        "FrozenEmpty".to_relationship_name(),
        "Frozen".to_relationship_name(),
        "Bounded".to_relationship_name(),
    ];
    let probe = crate::core_shared_objects::read_observations::CacheReadProbe::start(true);
    let first = cache.all_related_holons(&transactions, service.as_ref(), &id, &names, &[])?;
    let empty = first.get_collection_for_relationship(&names[0]).unwrap();
    assert!(empty.read().unwrap().get_members().is_empty());
    assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 1);
    let policies = service.policy_calls.load(Ordering::Relaxed);
    let preparation = service.preparation_calls.load(Ordering::Relaxed);
    service.edges.write().unwrap().insert((1, "Bounded".into()), vec![11]);
    service.clock.store(9, Ordering::Relaxed);
    let hit = cache.all_related_holons(&transactions, service.as_ref(), &id, &names, &[])?;
    assert!(Arc::ptr_eq(&empty, &hit.get_collection_for_relationship(&names[0]).unwrap()));
    assert!(hit
        .get_collection_for_relationship(&names[2])
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .is_empty());
    assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 1);
    service.clock.store(10, Ordering::Relaxed);
    let expired = cache.all_related_holons(&transactions, service.as_ref(), &id, &names, &[])?;
    assert!(Arc::ptr_eq(&empty, &expired.get_collection_for_relationship(&names[0]).unwrap()));
    assert_eq!(
        expired.get_collection_for_relationship(&names[2]).unwrap().read().unwrap().get_members(),
        &vec![CountingService::reference(&context, 11)]
    );
    assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 2);
    assert_eq!(service.policy_calls.load(Ordering::Relaxed), policies);
    assert_eq!(service.preparation_calls.load(Ordering::Relaxed), preparation);
    assert_eq!(service.count("Frozen"), 0);
    let observations = probe.finish();
    for (name, outcome, requests) in [
        ("FrozenEmpty", "missing", 1),
        ("FrozenEmpty", "hit", 2),
        ("Bounded", "missing", 1),
        ("Bounded", "hit", 1),
        ("Bounded", "expired", 1),
    ] {
        assert!(observations.iter().any(|entry| entry.source == id
            && entry.relationship.as_ref() == Some(&name.to_relationship_name())
            && entry.outcome == outcome
            && entry.requests == requests));
    }

    Ok(())
}

#[test]
fn bulk_failure_or_invalid_membership_installs_no_partial_entries() -> Result<(), HolonError> {
    for invalid in [false, true] {
        let service = Arc::new(CountingService::new(true));
        let space = HolonSpaceManager::new_with_managers(
            None,
            service.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        );
        let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let transactions = CacheReadTransactions { space: Arc::downgrade(&space) };
        let cache = crate::core_shared_objects::relationship_cache::RelationshipCache::new();
        let id = HolonId::Local(LocalId(vec![1]));
        let names = vec!["Frozen".to_relationship_name(), "Moving".to_relationship_name()];
        service.fail_reads.store(!invalid, Ordering::Relaxed);
        service.invalid_bulk.store(invalid, Ordering::Relaxed);
        assert!(cache
            .all_related_holons(&transactions, service.as_ref(), &id, &names, &[])
            .is_err());
        service.edges.write().unwrap().insert((1, "Frozen".into()), vec![11]);
        service.fail_reads.store(false, Ordering::Relaxed);
        service.invalid_bulk.store(false, Ordering::Relaxed);
        let result = cache.all_related_holons(&transactions, service.as_ref(), &id, &names, &[])?;
        assert_eq!(
            result
                .get_collection_for_relationship(&names[0])
                .unwrap()
                .read()
                .unwrap()
                .get_members(),
            &vec![CountingService::reference(&context, 11)]
        );
        assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 2);
    }
    Ok(())
}

#[test]
fn bulk_dance_fetches_one_source_snapshot_and_completes_empty_names() -> Result<(), HolonError> {
    use crate::dances::{DanceRequest, DanceType, RequestBody, ResponseBody};
    use crate::query_layer::{Node, NodeCollection};
    let service = Arc::new(CountingService::new(true));
    let space = HolonSpaceManager::new_with_managers(
        None,
        service.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let source = CountingService::reference(&context, 1);
    let names = vec!["Frozen".to_relationship_name(), "FrozenEmpty".to_relationship_name()];
    let request = DanceRequest::new(
        "fetch_all_related_holons".into(),
        DanceType::QueryMethod(NodeCollection {
            members: vec![Node::new(source, None)],
            query_spec: None,
        }),
        RequestBody::RelationshipSet { names: names.clone(), unresolved: vec![] },
    );
    let ResponseBody::SavedRelationshipSet(entries) =
        crate::dances::holon_dance_adapter::fetch_all_related_holons_dance(&context, request)?
    else {
        panic!("expected membership set");
    };
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].0, names[0]);
    assert_eq!(
        entries[0].1.collection.get_members(),
        &vec![CountingService::reference(&context, 10)]
    );
    assert_eq!(entries[1].0, names[1]);
    assert!(entries[1].1.collection.get_members().is_empty());
    assert_eq!(service.bulk_calls.load(Ordering::Relaxed), 1);
    assert!(service.calls.read().unwrap().is_empty(), "bulk transport must avoid named retrieval and descriptor discovery when policies are retained");
    Ok(())
}
