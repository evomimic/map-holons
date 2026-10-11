use crate::usage_transactions::UsageTransactions;
use base_types::{BaseValue, MapEnumValue, MapInteger, MapString};
use core_types::{HolonError, HolonId, LocalId, PropertyMap, PropertyName, RelationshipName};
use holons_core::core_shared_objects::{
    space_manager::HolonSpaceManager, transactions::TransactionContext, Holon, ReadableHolonState,
    RelationshipMap, SavedHolon,
};
use holons_core::{
    HolonCollection, HolonCollectionApi, HolonReference, HolonServiceApi, ReadableHolon,
    RelationshipCachePolicy, ServiceRoutingPolicy, StagedReference, TransientReference,
    WritableHolon,
};
use map_commands_contract::{
    VisualizerChoiceOrigin, VisualizerKind, VisualizerOwner, VisualizerSelectionRequest,
};
use std::{
    any::Any,
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// Persistent service double. The runtime still executes real reference/staging/commit APIs.
#[derive(Debug, Default)]
struct Store {
    nodes: Mutex<HashMap<u32, PropertyMap>>,
    edges: Mutex<HashMap<(u32, String), Vec<u32>>>,
    commits: Mutex<Vec<u64>>,
    relationship_reads: Mutex<Vec<(u32, String)>>,
    anchor_reads: Mutex<Vec<String>>,
    fail: AtomicBool,
    incomplete: AtomicBool,
}
fn id(n: u32) -> HolonId {
    HolonId::Local(LocalId(n.to_le_bytes().to_vec()))
}
fn number(id: &HolonId) -> u32 {
    u32::from_le_bytes(id.local_id().0.clone().try_into().unwrap())
}
fn reference(context: &Arc<TransactionContext>, n: u32) -> HolonReference {
    HolonReference::smart_from_id(context.space_read_handle(), id(n))
}
impl Store {
    fn edge(&self, n: u32, name: &str, targets: &[u32]) {
        let mut edges = self.edges.lock().unwrap();
        let old = edges.insert((n, name.into()), targets.to_vec()).unwrap_or_default();
        // Model committed inverse memberships used by descriptor and usage navigation.
        let inverse = match name {
            "DescribedBy" => "Instances",
            "UsesVisualizer" => "UsedByVisualizerUsage",
            "SourceType" => "SourceOf",
            _ => return,
        };
        for target in old {
            if let Some(members) = edges.get_mut(&(target, inverse.into())) {
                members.retain(|source| *source != n);
            }
        }
        for target in targets {
            let members = edges.entry((*target, inverse.into())).or_default();
            if !members.contains(&n) {
                members.push(n);
            }
        }
    }
    fn property(&self, n: u32, name: &str, value: BaseValue) {
        self.nodes.lock().unwrap().entry(n).or_default().insert(PropertyName(name.into()), value);
    }
}
impl HolonServiceApi for Store {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn relationship_cache_policy(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        _: &RelationshipName,
    ) -> Result<RelationshipCachePolicy, HolonError> {
        Ok(RelationshipCachePolicy::Fresh)
    }
    fn visualizer_artifact_available_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonReference,
    ) -> Result<bool, HolonError> {
        Ok(true)
    }
    fn get_saved_holon_by_key_internal(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<holons_core::SmartReference, HolonError> {
        self.anchor_reads.lock().unwrap().push(key.0.clone());
        let n = match key.0.as_str() {
            "VisualizerUsage.HolonType" => 100,
            "VisualizerSlotPreference.HolonType" => 101,
            "VisualizerDiscovery.Projection" => 150,
            "VisualizerDiscoveryLevel.Projection" => 151,
            "VisualizerDiscoveryCandidate.Projection" => 152,
            _ => return Err(HolonError::InvalidParameter(format!("Unexpected anchor {key}"))),
        };
        let HolonReference::Smart(value) = reference(context, n) else { unreachable!() };
        Ok(value)
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        source: &HolonId,
    ) -> Result<Holon, HolonError> {
        let props = self.nodes.lock().unwrap().get(&number(source)).cloned().unwrap_or_default();
        Ok(Holon::Saved(SavedHolon::new(source.local_id().clone(), props, None, MapInteger(1))))
    }
    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        self.relationship_reads.lock().unwrap().push((number(source), name.to_string()));
        let values = self
            .edges
            .lock()
            .unwrap()
            .get(&(number(source), name.to_string()))
            .cloned()
            .unwrap_or_default();
        let mut collection = HolonCollection::new_transient();
        collection.add_references(values.into_iter().map(|n| reference(context, n)).collect())?;
        Ok(collection)
    }
    fn fetch_all_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        let names: Vec<_> = self
            .edges
            .lock()
            .unwrap()
            .keys()
            .filter(|(n, _)| *n == number(source))
            .map(|(_, name)| name.clone())
            .collect();
        let mut map = RelationshipMap::new(HashMap::new());
        for name in names {
            let name = RelationshipName(name.into());
            map.insert(
                name.clone(),
                Arc::new(std::sync::RwLock::new(
                    self.fetch_related_holons_internal(context, source, &name)?,
                )),
            );
        }
        Ok(map)
    }
    fn get_all_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        let mut collection = HolonCollection::new_transient();
        let ids: Vec<_> = self.nodes.lock().unwrap().keys().copied().collect();
        collection.add_references(ids.into_iter().map(|n| reference(context, n)).collect())?;
        Ok(collection)
    }
    fn commit_internal(
        &self,
        context: &Arc<TransactionContext>,
        staged: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        if self.fail.swap(false, Ordering::SeqCst) {
            return Err(HolonError::Misc("injected commit failure".into()));
        }
        self.commits.lock().unwrap().push(context.tx_id().value());
        for staged in staged {
            let model = staged.get_holon_to_commit(context)?;
            let mut model = model.write().unwrap();
            let Holon::Staged(holon) = &mut *model else { unreachable!() };
            let n = match holon.versioned_source_id_ref() {
                Some(source) => number(&HolonId::Local(source.clone())),
                None => holon.holon_id().map(|value| number(&value)).unwrap_or_else(|_| {
                    self.nodes.lock().unwrap().keys().max().copied().unwrap_or(199) + 1
                }),
            };
            let properties = holon.holon_clone_model().properties;
            self.nodes.lock().unwrap().insert(n, properties);
            if holon.holon_id().is_err() {
                holon.to_committed(id(n).local_id().clone())?;
            }
        }
        // Persist outgoing relationships after every staged target has its saved identity.
        for staged in staged {
            let source = number(&staged.holon_id()?);
            for (name, collection) in staged.all_related_holons()?.iter() {
                let targets = collection
                    .read()
                    .unwrap()
                    .get_members()
                    .iter()
                    .map(|r| r.holon_id().map(|id| number(&id)))
                    .collect::<Result<Vec<_>, _>>()?;
                self.edge(source, &name.to_string(), &targets);
            }
        }
        let mut response = context.mutation().new_holon(Some("commit-response".into()))?;
        response.with_property_value(
            "CommitRequestStatus",
            if self.incomplete.swap(false, Ordering::SeqCst) { "Incomplete" } else { "Complete" },
        )?;
        Ok(response)
    }
    fn delete_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &LocalId,
    ) -> Result<(), HolonError> {
        unreachable!()
    }
    fn load_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        unreachable!()
    }
}
fn setup() -> (Arc<Store>, Arc<HolonSpaceManager>, Arc<TransactionContext>) {
    let store = Arc::new(Store::default());
    for n in [1, 2, 3, 4, 5, 10, 20, 21, 30, 31, 40, 60, 61, 70, 100, 101] {
        store.nodes.lock().unwrap().insert(n, PropertyMap::new());
    }
    for (source, name, targets) in [
        (2, "DescribedBy", vec![1]),
        (3, "DescribedBy", vec![1]),
        (4, "DescribedBy", vec![5]),
        (1, "HasApplicableVisualizer", vec![20, 21]),
        (5, "HasApplicableVisualizer", vec![20, 21]),
        (20, "DescribedBy", vec![10]),
        (21, "DescribedBy", vec![10]),
        (30, "AcceptsVisualizerType", vec![10]),
        (31, "AcceptsVisualizerType", vec![10]),
        (40, "HasSlot", vec![30, 31]),
        (60, "ForMetaDesignSystem", vec![61]),
        (20, "ImplementedBy", vec![70]),
        (21, "ImplementedBy", vec![70]),
    ] {
        store.edge(source, name, &targets);
    }
    for (name, value) in [
        ("VisualizerModuleFormat", "ESModule"),
        ("VisualizerImplementationKey", "fixture"),
        ("Entrypoint", "default"),
        ("VisualizerArtifactDigest", "sha256:fixture"),
    ] {
        store.property(70, name, BaseValue::StringValue(value.into()));
    }
    store.property(
        70,
        "VisualizerImplementationRuntime",
        BaseValue::EnumValue(MapEnumValue("TypeScript".into())),
    );
    store.property(111, "TypeName", BaseValue::StringValue("InverseRelationshipType".into()));
    store.property(112, "TypeName", BaseValue::StringValue("UsedByVisualizerUsage".into()));
    store.edge(112, "Extends", &[111]);
    store.edge(112, "SourceType", &[10]);
    store.edge(112, "TargetType", &[100]);
    // Saved cloning retains only relationships declared by the source descriptor.
    store.property(110, "TypeName", BaseValue::StringValue("DeclaredRelationshipType".into()));
    for (descriptor, names) in [
        (
            100,
            vec![
                "DescribedBy",
                "UsesVisualizer",
                "ForSubjectType",
                "SelectedForSlot",
                "PropertySalienceOverrides",
                "RelationshipSalienceOverrides",
                "ActionSalienceOverrides",
                "ActionGroupOverrides",
            ],
        ),
        (
            101,
            vec!["DescribedBy", "PreferenceForSlot", "PreferenceForSubjectType", "PreferredUsage"],
        ),
    ] {
        let mut declarations = Vec::new();
        for name in names {
            let n = store.nodes.lock().unwrap().keys().max().copied().unwrap() + 1;
            store.property(n, "TypeName", BaseValue::StringValue(name.into()));
            store.property(n, "IsDefinitional", BaseValue::BooleanValue(false.into()));
            store.property(n, "AllowsDuplicates", BaseValue::BooleanValue(false.into()));
            store.edge(n, "Extends", &[110]);
            declarations.push(n);
        }
        store.edge(descriptor, "InstanceRelationships", &declarations);
    }
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        store.clone(),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    (store, space, context)
}
fn request(
    context: &Arc<TransactionContext>,
    subject: u32,
    slot: u32,
) -> VisualizerSelectionRequest {
    VisualizerSelectionRequest {
        subject: reference(context, subject),
        requested_kind: VisualizerKind::Node,
        owner: VisualizerOwner::Visualizer(reference(context, 40)),
        slot: reference(context, slot),
        theme: reference(context, 60),
    }
}

#[test]
fn finding_usage_never_initializes_or_commits_and_preserves_existing_configuration(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let usages = UsageTransactions::default();
    for _ in 0..2 {
        assert!(usages.find(&caller, request(&caller, 2, 30), reference(&caller, 20))?.is_none());
    }
    assert!(store.commits.lock().unwrap().is_empty());
    let initialized =
        usages.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    assert!(initialized.initialized);
    let commits = store.commits.lock().unwrap().len();
    let found = usages.find(&caller, request(&caller, 2, 30), reference(&caller, 20))?.unwrap();
    assert!(!found.initialized);
    assert_eq!(found.usage.holon_id()?, initialized.usage.holon_id()?);
    assert_eq!(store.commits.lock().unwrap().len(), commits);
    Ok(())
}

#[test]
fn finding_usage_does_not_resume_a_failed_initialization_commit() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let usages = UsageTransactions::default();
    store.fail.store(true, Ordering::SeqCst);
    assert!(usages
        .select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))
        .is_err());
    assert!(usages.find(&caller, request(&caller, 2, 30), reference(&caller, 20))?.is_none());
    assert!(store.commits.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn finding_usage_does_not_expose_or_retry_a_partially_persisted_initialization(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let usages = UsageTransactions::default();
    store.incomplete.store(true, Ordering::SeqCst);
    assert!(usages
        .select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))
        .is_err());
    let commits = store.commits.lock().unwrap().len();
    assert!(commits > 0);
    assert!(usages.find(&caller, request(&caller, 2, 30), reference(&caller, 20))?.is_none());
    assert_eq!(store.commits.lock().unwrap().len(), commits);
    usages.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    assert!(usages.find(&caller, request(&caller, 2, 30), reference(&caller, 20))?.is_some());
    Ok(())
}

#[test]
fn discovery_projection_resolves_each_record_type_once() -> Result<(), HolonError> {
    use crate::discovery_evidence::DiscoveryEvidence;
    use map_commands_contract::{
        DiscoveryStopReason, VisualizerAssessment, VisualizerCandidate, VisualizerDiscovery,
    };
    let (store, space, source) = setup();
    let evidence = DiscoveryEvidence::default();
    let captured = VisualizerDiscovery {
        candidates: (20..40)
            .map(|n| VisualizerCandidate {
                visualizer: reference(&source, n),
                declared_on: vec![reference(&source, 1)],
                assessment: VisualizerAssessment::Viable,
            })
            .collect(),
        current_selection: None,
        ancestry: (1..5).map(|n| reference(&source, n)).collect(),
        stop_reason: DiscoveryStopReason::LineageExhausted,
        snapshot: None,
    };
    let key = evidence.retain(source.tx_id(), request(&source, 2, 30), &captured)?;
    let destination = space.get_transaction_manager().open_public_transaction(space.clone())?;
    store.anchor_reads.lock().unwrap().clear();
    let projection = evidence.project(&destination, &key)?;
    assert_eq!(
        projection.related_holons("DiscoveryCandidates")?.read().unwrap().get_count(),
        MapInteger(20)
    );
    assert_eq!(
        store.anchor_reads.lock().unwrap().len(),
        3,
        "Projection descriptor lookups must not grow with the number of captured records"
    );
    Ok(())
}

#[test]
fn discovery_projection_consumes_captured_facts_without_rediscovery() -> Result<(), HolonError> {
    use crate::discovery_evidence::DiscoveryEvidence;
    use map_commands_contract::{
        DiscoveryStopReason, VisualizerAssessment, VisualizerCandidate, VisualizerDiscovery,
    };
    let (store, space, source) = setup();
    let evidence = DiscoveryEvidence::default();
    let candidate = VisualizerCandidate {
        visualizer: reference(&source, 20),
        declared_on: vec![reference(&source, 1)],
        assessment: VisualizerAssessment::Viable,
    };
    let captured = VisualizerDiscovery {
        candidates: vec![candidate.clone()],
        current_selection: Some(candidate),
        ancestry: vec![reference(&source, 1)],
        stop_reason: DiscoveryStopReason::LineageExhausted,
        snapshot: None,
    };
    let key = evidence.retain(source.tx_id(), request(&source, 2, 30), &captured)?;
    // Later graph changes cannot alter the recorded candidate/declaration assessment.
    store.edge(1, "HasApplicableVisualizer", &[]);
    let destination = space.get_transaction_manager().open_public_transaction(space.clone())?;
    assert!(evidence.release(&destination.tx_id(), &key).is_err());
    store.relationship_reads.lock().unwrap().clear();
    let projection = evidence.project(&destination, &key)?;
    let levels = projection.related_holons("DiscoveryLevels")?;
    assert_eq!(levels.read().unwrap().get_count(), MapInteger(1));
    assert_eq!(
        levels
            .read()
            .unwrap()
            .get_by_index(0)?
            .property_value(&PropertyName("DiscoveryLevelIndex".into()))?,
        Some(BaseValue::IntegerValue(MapInteger(0)))
    );
    let candidates = projection.related_holons("DiscoveryCandidates")?;
    assert_eq!(candidates.read().unwrap().get_count(), MapInteger(1));
    assert_eq!(
        candidates
            .read()
            .unwrap()
            .get_by_index(0)?
            .property_value(&PropertyName("DiscoveryAssessment".into()))?,
        Some(BaseValue::StringValue("viable".into()))
    );
    assert_eq!(
        projection.property_value(&PropertyName("DiscoveryStopReason".into()))?,
        Some(BaseValue::StringValue("lineage_exhausted".into()))
    );
    assert!(!store
        .relationship_reads
        .lock()
        .unwrap()
        .iter()
        .any(|(source, name)| name == "HasApplicableVisualizer"
            || (*source == 1 && name == "Extends")));
    assert!(evidence.project(&destination, &key).is_err());
    evidence.release(&source.tx_id(), &key)?;
    let key = evidence.retain(source.tx_id(), request(&source, 2, 30), &captured)?;
    evidence.release_transaction(&source.tx_id())?;
    assert!(evidence.project(&destination, &key).is_err());
    Ok(())
}
#[test]
fn usage_persists_independently_and_reuses_by_type_across_instances_slots_and_restart(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    let edit = caller.mutation().new_holon(Some("unrelated-edit".into()))?;
    caller.mutation().stage_new_holon(edit)?;
    let result =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    assert!(result.initialized);
    assert!(matches!(result.usage, HolonReference::Smart(_)));
    assert!(caller.is_open());
    assert_eq!(caller.lookup().staged_count()?, 1);
    assert!(!store.commits.lock().unwrap().contains(&caller.tx_id().value()));
    let usage = dahn_selection::usage::VisualizerUsage::from_holon(&caller, result.usage.clone())?;
    assert!(usage.property_salience_overrides()?.is_empty());
    assert!(usage.relationship_salience_overrides()?.is_empty());
    assert!(usage.action_salience_overrides()?.is_empty());
    assert!(usage.action_group_overrides()?.is_empty());
    assert!(usage.selected_slots()?.is_empty());
    let reused =
        service.select(&space, &caller, request(&caller, 3, 31), reference(&caller, 20))?;
    assert!(!reused.initialized);
    assert_eq!(result.usage, reused.usage);
    let person =
        service.select(&space, &caller, request(&caller, 4, 30), reference(&caller, 20))?;
    assert_ne!(result.usage, person.usage);
    let restarted = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        store,
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let caller2 = restarted.get_transaction_manager().open_public_transaction(restarted.clone())?;
    let reused = UsageTransactions::default().select(
        &restarted,
        &caller2,
        request(&caller2, 2, 30),
        reference(&caller2, 20),
    )?;
    assert!(!reused.initialized);
    assert_eq!(reused.usage.holon_id()?, result.usage.holon_id()?);
    Ok(())
}
#[test]
fn failed_and_incomplete_commits_retry_same_work_without_duplicates() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.fail.store(true, Ordering::SeqCst);
    assert!(service
        .select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))
        .is_err());
    store.incomplete.store(true, Ordering::SeqCst);
    assert!(service
        .select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))
        .is_err());
    let result =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    assert!(result.initialized);
    assert_eq!(dahn_selection::usage::instances(&caller, "VisualizerUsage.HolonType")?.len(), 1);
    assert_eq!(caller.lookup().staged_count()?, 0);
    assert!(caller.is_open());
    Ok(())
}
#[test]
fn concurrent_initializers_converge() {
    let (_, space, caller) = setup();
    let service = Arc::new(UsageTransactions::default());
    let joins: Vec<_> = (0..4)
        .map(|_| {
            let space = space.clone();
            let caller = caller.clone();
            let service = service.clone();
            std::thread::spawn(move || {
                service
                    .select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))
                    .unwrap()
            })
        })
        .collect();
    let results: Vec<_> = joins.into_iter().map(|join| join.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.initialized).count(), 1);
    assert!(results.iter().all(|r| r.usage == results[0].usage));
}
#[test]
fn successful_explicit_use_remembers_preference_exploration_does_not() -> Result<(), HolonError> {
    let (_, space, caller) = setup();
    let service = UsageTransactions::default();
    let first = service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        first.usage.clone(),
        VisualizerChoiceOrigin::Explicit,
        report(&service),
    )?;
    let alternate =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 21))?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 21),
        alternate.usage,
        VisualizerChoiceOrigin::Exploratory,
        report(&service),
    )?;
    let preferences =
        dahn_selection::usage::instances(&caller, "VisualizerSlotPreference.HolonType")?;
    assert_eq!(preferences.len(), 1);
    assert_eq!(dahn_selection::usage::single(&preferences[0], "PreferredUsage")?, first.usage);
    assert_eq!(
        dahn_selection::usage::VisualizerUsage::from_holon(&caller, first.usage.clone())?
            .selected_slots()?,
        vec![reference(&caller, 30)]
    );
    assert!(service
        .record(
            &space,
            &caller,
            request(&caller, 4, 30),
            reference(&caller, 20),
            first.usage,
            VisualizerChoiceOrigin::Explicit,
            report(&service),
        )
        .is_err());
    Ok(())
}

#[test]
fn existing_configuration_and_slot_association_win_over_ambiguous_matches() -> Result<(), HolonError>
{
    let (store, space, caller) = setup();
    for n in [200, 201] {
        store.property(n, "Key", BaseValue::StringValue(format!("usage-{n}").into()));
        store.edge(n, "DescribedBy", &[100]);
        store.edge(n, "UsesVisualizer", &[20]);
        store.edge(n, "ForSubjectType", &[1]);
    }
    store.edge(200, "PropertySalienceOverrides", &[70]);
    let service = UsageTransactions::default();
    assert!(service
        .select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))
        .is_err());
    store.edge(200, "SelectedForSlot", &[30]);
    let result =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    assert!(!result.initialized);
    assert_eq!(result.usage, reference(&caller, 200));
    assert_eq!(
        dahn_selection::usage::VisualizerUsage::from_holon(&caller, result.usage)?
            .property_salience_overrides()?,
        vec![reference(&caller, 70)]
    );
    assert!(store.commits.lock().unwrap().is_empty());
    store.edge(201, "SelectedForSlot", &[30]);
    assert!(service
        .select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))
        .is_err());
    Ok(())
}
#[test]
fn incomplete_outcome_finishes_before_a_new_preference_supersedes_it() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    let first = service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let second =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 21))?;
    store.incomplete.store(true, Ordering::SeqCst);
    assert!(service
        .record(
            &space,
            &caller,
            request(&caller, 2, 30),
            reference(&caller, 20),
            first.usage,
            VisualizerChoiceOrigin::Explicit,
            report(&service),
        )
        .is_err());
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 21),
        second.usage.clone(),
        VisualizerChoiceOrigin::Explicit,
        report(&service),
    )?;
    let preferences =
        dahn_selection::usage::instances(&caller, "VisualizerSlotPreference.HolonType")?;
    assert_eq!(preferences.len(), 1);
    assert_eq!(dahn_selection::usage::single(&preferences[0], "PreferredUsage")?, second.usage);
    Ok(())
}

fn report(service: &UsageTransactions) -> map_commands_contract::VisualizerUseReport {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    map_commands_contract::VisualizerUseReport {
        session: service.report_session.clone(),
        occurrence_id: "test-node".into(),
        sequence: SEQUENCE.fetch_add(1, Ordering::SeqCst),
    }
}

#[test]
fn delayed_choice_from_another_occurrence_cannot_undo_newer_preference() -> Result<(), HolonError> {
    let (_, space, caller) = setup();
    let service = UsageTransactions::default();
    let first = service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let second =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 21))?;
    let old = report(&service);
    let mut newer = report(&service);
    newer.occurrence_id = "another-node".into();
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 21),
        second.usage.clone(),
        VisualizerChoiceOrigin::Explicit,
        newer.clone(),
    )?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        first.usage.clone(),
        VisualizerChoiceOrigin::Explicit,
        old,
    )?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 21),
        second.usage.clone(),
        VisualizerChoiceOrigin::Explicit,
        newer.clone(),
    )?;
    let preferences =
        dahn_selection::usage::instances(&caller, "VisualizerSlotPreference.HolonType")?;
    assert_eq!(preferences.len(), 1);
    assert_eq!(dahn_selection::usage::single(&preferences[0], "PreferredUsage")?, second.usage);
    assert!(service
        .record(
            &space,
            &caller,
            request(&caller, 2, 30),
            reference(&caller, 20),
            first.usage.clone(),
            VisualizerChoiceOrigin::Explicit,
            newer
        )
        .is_err());
    assert_eq!(
        dahn_selection::usage::VisualizerUsage::from_holon(&caller, first.usage.clone())?
            .selected_slots()?,
        vec![reference(&caller, 30)]
    );
    let mut expired = report(&service);
    expired.session = "expired".into();
    assert!(service
        .record(
            &space,
            &caller,
            request(&caller, 2, 30),
            reference(&caller, 21),
            second.usage,
            VisualizerChoiceOrigin::Explicit,
            expired
        )
        .is_err());
    Ok(())
}

#[test]
fn usage_lookup_reads_only_selected_visualizer_usages() -> Result<(), HolonError> {
    let (store, _, context) = setup();
    // A usage of another Visualizer deliberately lacks ForSubjectType.
    // A global usage scan would inspect it and fail.
    store.edge(200, "DescribedBy", &[100]);
    store.edge(200, "UsesVisualizer", &[21]);
    store.edge(201, "DescribedBy", &[100]);
    store.edge(201, "UsesVisualizer", &[20]);
    store.edge(201, "ForSubjectType", &[1]);
    let found = dahn_selection::usage::find_usage(
        &reference(&context, 20),
        &reference(&context, 1),
        &reference(&context, 30),
    )?
    .expect("matching usage");
    assert_eq!(found.holon(), &reference(&context, 201));
    assert_eq!(
        *store.relationship_reads.lock().unwrap(),
        vec![(20, "UsedByVisualizerUsage".into()), (201, "ForSubjectType".into()),]
    );
    assert!(store.anchor_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn usage_creation_uses_relationship_target_type_without_key_lookup() -> Result<(), HolonError> {
    let (store, _, context) = setup();
    // The selected Visualizer's type inherits its usage relationship.
    store.edge(10, "Extends", &[9]);
    store.edge(112, "SourceType", &[9]);
    // A noncanonical descriptor identity proves creation follows TargetType.
    store.property(200, "TypeName", BaseValue::StringValue("FixtureUsage".into()));
    store.edge(112, "TargetType", &[200]);
    let usage = dahn_selection::usage::initialize_usage(
        &context,
        reference(&context, 20),
        reference(&context, 1),
    )?;
    let usage: HolonReference = usage.into();
    assert_eq!(dahn_selection::usage::single(&usage, "DescribedBy")?, reference(&context, 200));
    assert_eq!(dahn_selection::usage::single(&usage, "UsesVisualizer")?, reference(&context, 20));
    assert_eq!(dahn_selection::usage::single(&usage, "ForSubjectType")?, reference(&context, 1));
    assert!(store.anchor_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn usage_creation_rejects_missing_relationship_target_type() {
    let (store, _, context) = setup();
    store.edge(112, "TargetType", &[]);
    assert!(dahn_selection::usage::initialize_usage(
        &context,
        reference(&context, 20),
        reference(&context, 1),
    )
    .is_err());
    assert!(store.commits.lock().unwrap().is_empty());
    assert!(store.anchor_reads.lock().unwrap().is_empty());
}
