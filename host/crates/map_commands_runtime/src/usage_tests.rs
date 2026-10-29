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
    usage_lookups: Mutex<Vec<[u32; 3]>>,
    fail: AtomicBool,
    unavailable: AtomicBool,
    rejected: AtomicBool,
    incomplete: AtomicBool,
    block_commit: AtomicBool,
    artifact_missing: AtomicBool,
    commit_gate: Mutex<bool>,
    commit_changed: std::sync::Condvar,
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
    fn construct_effective_holon_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<holons_core::EffectiveHolonDescriptor, HolonError> {
        crate::prepared_descriptor_fixture::construct(context, target)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn relationship_cache_policy(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        _: &RelationshipName,
    ) -> Result<Option<RelationshipCachePolicy>, HolonError> {
        Ok(Some(RelationshipCachePolicy::Fresh))
    }
    fn visualizer_artifact_available_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonReference,
    ) -> Result<bool, HolonError> {
        Ok(!self.artifact_missing.load(Ordering::SeqCst))
    }
    fn resolve_visualizer_usage_internal(
        &self,
        _: &Arc<TransactionContext>,
        visualizer: &HolonReference,
        subject_type: &HolonReference,
        slot: &HolonReference,
    ) -> Result<holons_core::dances::VisualizerUsageResolution, HolonError> {
        self.usage_lookups.lock().unwrap().push([
            number(&visualizer.holon_id()?),
            number(&subject_type.holon_id()?),
            number(&slot.holon_id()?),
        ]);
        holons_core::dances::visualizer_usage_resolution::resolve_visualizer_usage(
            visualizer,
            subject_type,
            slot,
        )
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
            "VisualizerUsageResolution.Projection" => 153,
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
        if self.block_commit.load(Ordering::SeqCst) {
            let mut entered = self.commit_gate.lock().unwrap();
            *entered = true;
            self.commit_changed.notify_all();
            while self.block_commit.load(Ordering::SeqCst) {
                entered = self.commit_changed.wait(entered).unwrap();
            }
        }
        if self.unavailable.swap(false, Ordering::SeqCst) {
            return Err(HolonError::ServiceNotAvailable("offline".into()));
        }
        if self.rejected.swap(false, Ordering::SeqCst) {
            let mut response = context.mutation().new_holon(Some("rejected-response".into()))?;
            response.with_property_value("CommitRequestStatus", "Rejected")?;
            return Ok(response);
        }
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
        (40, "DescribedBy", vec![41]),
        (41, "HasSlot", vec![30, 31]),
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
    for slot in [30, 31] {
        store.property(
            slot,
            "CompositionRole",
            BaseValue::StringValue(format!("role-{slot}").into()),
        );
    }
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

fn wait_saved(service: &UsageTransactions, id: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match service.status(id).unwrap() {
            crate::usage_persistence::SaveStatus::Saved => return,
            crate::usage_persistence::SaveStatus::Failed(error)
            | crate::usage_persistence::SaveStatus::ServicePaused(error) => {
                panic!("background save failed: {error}")
            }
            _ => assert!(std::time::Instant::now() < deadline, "save did not complete"),
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
fn usage_report(
    service: &UsageTransactions,
    sequence: u64,
) -> map_commands_contract::VisualizerUseReport {
    map_commands_contract::VisualizerUseReport {
        session: service.report_session.clone(),
        occurrence_id: format!("node-{sequence}"),
        sequence,
    }
}

#[test]
fn startup_round_returns_live_usages_then_commits_one_independent_batch() -> Result<(), HolonError>
{
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    let unrelated = caller.mutation().new_holon(Some("foreground-edit".into()))?;
    caller.mutation().stage_new_holon(unrelated)?;
    service.begin(&space, &caller)?;
    let first = service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let second =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 21))?;
    assert!(matches!(first.usage, HolonReference::Transient(_)));
    assert!(matches!(second.usage, HolonReference::Transient(_)));
    assert!(store.commits.lock().unwrap().is_empty(), "selection must not commit");
    assert_eq!(caller.lookup().staged_count()?, 1);
    let id = service.submit(&caller)?.unwrap();
    wait_saved(&service, id);
    assert_eq!(store.commits.lock().unwrap().len(), 1);
    assert_ne!(store.commits.lock().unwrap()[0], caller.tx_id().value());
    assert!(caller.is_open());
    assert_eq!(caller.lookup().staged_count()?, 1);
    assert_eq!(dahn_selection::usage::instances(&caller, "VisualizerUsage.HolonType")?.len(), 2);
    Ok(())
}

#[test]
fn blocked_backend_does_not_block_selection_lookup_or_queue_acceptance() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.block_commit.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let first = service.submit(&caller)?.unwrap();
    {
        let gate = store.commit_gate.lock().unwrap();
        let (_gate, timeout) = store
            .commit_changed
            .wait_timeout_while(gate, std::time::Duration::from_secs(2), |entered| !*entered)
            .unwrap();
        assert!(!timeout.timed_out(), "worker never entered commit");
    }
    // Commit is held behind a gate until AFTER these foreground operations finish.
    let another = space.get_transaction_manager().open_public_transaction(space.clone())?;
    assert!(service.find(&another, request(&another, 2, 30), reference(&another, 20))?.is_none());
    service.begin(&space, &another)?;
    let usage =
        service.select(&space, &another, request(&another, 2, 30), reference(&another, 21))?;
    assert!(usage.initialized);
    let second = service.submit(&another)?.unwrap();
    assert_eq!(service.status(second)?, crate::usage_persistence::SaveStatus::Queued);
    store.block_commit.store(false, Ordering::SeqCst);
    store.commit_changed.notify_all();
    wait_saved(&service, first);
    wait_saved(&service, second);
    Ok(())
}

#[test]
fn closing_source_context_after_acceptance_preserves_snapshot() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.block_commit.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let id = service.submit(&caller)?.unwrap();
    caller.dispose()?;
    store.block_commit.store(false, Ordering::SeqCst);
    store.commit_changed.notify_all();
    wait_saved(&service, id);
    let next = space.get_transaction_manager().open_public_transaction(space.clone())?;
    assert!(service.find(&next, request(&next, 2, 30), reference(&next, 20))?.is_some());
    Ok(())
}

#[test]
fn later_live_revision_updates_same_durable_usage_without_overwriting_live_state(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let first = service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let id = service.submit(&caller)?.unwrap();
    wait_saved(&service, id);
    let saved =
        service.find(&caller, request(&caller, 2, 30), reference(&caller, 20))?.unwrap().usage;
    let mut live = first.usage.clone();
    live.with_property_value("Key", "updated-live")?;
    service.begin(&space, &caller)?;
    service.capture(&caller, &live)?;
    let id = service.submit(&caller)?.unwrap();
    live.with_property_value("Key", "newer-unsaved")?;
    wait_saved(&service, id);
    assert_eq!(live.key()?, Some("newer-unsaved".into()));
    assert_eq!(
        store.nodes.lock().unwrap()[&number(&saved.holon_id()?)][&PropertyName("Key".into())],
        BaseValue::StringValue("updated-live".into())
    );
    assert_eq!(dahn_selection::usage::instances(&caller, "VisualizerUsage.HolonType")?.len(), 1);
    assert_eq!(store.commits.lock().unwrap().len(), 2);
    Ok(())
}

#[test]
fn usage_and_successful_use_share_one_commit_and_explicit_preference() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let first = service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        first.usage.clone(),
        VisualizerChoiceOrigin::Explicit,
        usage_report(&service, 2),
    )?;
    let second =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 21))?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 21),
        second.usage,
        VisualizerChoiceOrigin::Exploratory,
        usage_report(&service, 3),
    )?;
    let id = service.submit(&caller)?.unwrap();
    wait_saved(&service, id);
    let usage =
        service.find(&caller, request(&caller, 2, 30), reference(&caller, 20))?.unwrap().usage;
    let preferences =
        dahn_selection::usage::instances(&caller, "VisualizerSlotPreference.HolonType")?;
    assert_eq!(preferences.len(), 1);
    assert_eq!(single_preferred(&preferences[0])?, usage);
    assert_eq!(
        dahn_selection::usage::related(&usage, "SelectedForSlot")?,
        vec![reference(&caller, 30)]
    );
    assert_eq!(store.commits.lock().unwrap().len(), 1);
    Ok(())
}
fn single_preferred(reference: &HolonReference) -> Result<HolonReference, HolonError> {
    dahn_selection::usage::single(reference, "PreferredUsage")
}

#[test]
fn incomplete_commit_resumes_same_transaction_without_duplicate_creation() -> Result<(), HolonError>
{
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.incomplete.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let id = service.submit(&caller)?.unwrap();
    wait_saved(&service, id);
    let commits = store.commits.lock().unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0], commits[1]);
    assert_eq!(dahn_selection::usage::instances(&caller, "VisualizerUsage.HolonType")?.len(), 1);
    Ok(())
}

#[test]
fn unknown_failure_is_retained_for_explicit_retry_without_automatic_repetition(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.fail.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let id = service.submit(&caller)?.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !matches!(service.status(id)?, crate::usage_persistence::SaveStatus::Failed(_)) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(store.commits.lock().unwrap().is_empty());
    service.retry(id)?;
    wait_saved(&service, id);
    assert_eq!(dahn_selection::usage::instances(&caller, "VisualizerUsage.HolonType")?.len(), 1);
    Ok(())
}

#[test]
fn service_outage_pauses_following_batches_without_repeated_backend_attempts(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.unavailable.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let first = service.submit(&caller)?.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !matches!(service.status(first)?, crate::usage_persistence::SaveStatus::ServicePaused(_))
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let other = space.get_transaction_manager().open_public_transaction(space.clone())?;
    service.begin(&space, &other)?;
    service.select(&space, &other, request(&other, 2, 30), reference(&other, 21))?;
    let second = service.submit(&other)?.unwrap();
    assert_eq!(service.status(second)?, crate::usage_persistence::SaveStatus::Queued);
    assert!(store.commits.lock().unwrap().is_empty());
    service.retry(first)?;
    wait_saved(&service, first);
    wait_saved(&service, second);
    assert_eq!(store.commits.lock().unwrap().len(), 2);
    Ok(())
}

#[test]
fn rejected_commit_does_not_enter_automatic_incomplete_retry_policy() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.rejected.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let id = service.submit(&caller)?.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if let crate::usage_persistence::SaveStatus::Failed(reason) = service.status(id)? {
            assert!(reason.contains("rejected"), "{reason}");
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(store.commits.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn queued_updates_coalesce_and_continue_after_creation_without_another_gesture(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.block_commit.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    let selection =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let first = service.submit(&caller)?.unwrap();
    let mut live = selection.usage;
    service.begin(&space, &caller)?;
    live.with_property_value("Key", "intermediate")?;
    service.capture(&caller, &live)?;
    live.with_property_value("Key", "latest")?;
    service.capture(&caller, &live)?;
    let second = service.submit(&caller)?.unwrap();
    store.block_commit.store(false, Ordering::SeqCst);
    store.commit_changed.notify_all();
    wait_saved(&service, first);
    wait_saved(&service, second);
    let members = dahn_selection::usage::instances(&caller, "VisualizerUsage.HolonType")?;
    assert_eq!(members.len(), 1);
    assert_eq!(
        store.nodes.lock().unwrap()[&number(&members[0].holon_id()?)][&PropertyName("Key".into())],
        BaseValue::StringValue("latest".into())
    );
    assert_eq!(store.commits.lock().unwrap().len(), 2);
    Ok(())
}

#[test]
fn stale_explicit_report_preserves_successful_use_without_replacing_latest_choice(
) -> Result<(), HolonError> {
    let (_store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let latest =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        latest.usage,
        VisualizerChoiceOrigin::Explicit,
        usage_report(&service, 2),
    )?;
    let stale = service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 21))?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 21),
        stale.usage,
        VisualizerChoiceOrigin::Explicit,
        usage_report(&service, 1),
    )?;
    let id = service.submit(&caller)?.unwrap();
    wait_saved(&service, id);
    let latest =
        service.find(&caller, request(&caller, 2, 30), reference(&caller, 20))?.unwrap().usage;
    let stale =
        service.find(&caller, request(&caller, 2, 30), reference(&caller, 21))?.unwrap().usage;
    let preferences =
        dahn_selection::usage::instances(&caller, "VisualizerSlotPreference.HolonType")?;
    assert_eq!(preferences.len(), 1);
    assert_eq!(single_preferred(&preferences[0])?, latest);
    assert_eq!(
        dahn_selection::usage::related(&stale, "SelectedForSlot")?,
        vec![reference(&caller, 30)]
    );
    Ok(())
}

#[test]
fn local_failure_does_not_block_an_unrelated_usage_batch() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.fail.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let failed = service.submit(&caller)?.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !matches!(service.status(failed)?, crate::usage_persistence::SaveStatus::Failed(_)) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let other = space.get_transaction_manager().open_public_transaction(space.clone())?;
    service.begin(&space, &other)?;
    service.select(&space, &other, request(&other, 2, 30), reference(&other, 21))?;
    let next = service.submit(&other)?.unwrap();
    wait_saved(&service, next);
    assert!(matches!(service.status(failed)?, crate::usage_persistence::SaveStatus::Failed(_)));
    assert_eq!(store.commits.lock().unwrap().len(), 1);
    Ok(())
}

#[test]
fn live_usage_snapshot_capture_does_not_resolve_descriptors() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let selection =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    store.relationship_reads.lock().unwrap().clear();
    store.anchor_reads.lock().unwrap().clear();
    service.capture(&caller, &selection.usage)?;
    assert!(
        store.relationship_reads.lock().unwrap().is_empty(),
        "Snapshot capture must copy live state without descriptor resolution"
    );
    assert!(store.anchor_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn prepared_successful_use_capture_does_not_repeat_selection_or_snapshot_resolution(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let selection =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    store.relationship_reads.lock().unwrap().clear();
    store.anchor_reads.lock().unwrap().clear();
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        selection.usage,
        VisualizerChoiceOrigin::Automatic,
        usage_report(&service, 1),
    )?;
    assert!(
        store.relationship_reads.lock().unwrap().is_empty(),
        "Recording a prepared presentation must not repeat selection or clone completion"
    );
    assert!(store.anchor_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn usage_subject_type_resolution_does_not_repeat_owner_and_theme_assessment(
) -> Result<(), HolonError> {
    let (store, _space, caller) = setup();
    assert_eq!(
        dahn_selection::usage_subject_type(&request(&caller, 2, 30))?,
        reference(&caller, 1)
    );
    assert!(
        !store
            .relationship_reads
            .lock()
            .unwrap()
            .iter()
            .any(|(source, _)| [30, 40, 60].contains(source)),
        "Resolving the semantic descriptor must not reassess the selection owner, slot, and theme"
    );
    Ok(())
}

#[test]
fn repeated_automatic_presentation_does_not_commit_an_unchanged_usage() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    for sequence in 1..=2 {
        service.begin(&space, &caller)?;
        let selected =
            service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
        service.record(
            &space,
            &caller,
            request(&caller, 2, 30),
            reference(&caller, 20),
            selected.usage,
            VisualizerChoiceOrigin::Automatic,
            usage_report(&service, sequence),
        )?;
        let batch = service.submit(&caller)?.unwrap();
        wait_saved(&service, batch);
    }
    assert_eq!(
        store.commits.lock().unwrap().len(),
        1,
        "An unchanged successful-use association needs no second backend transaction"
    );
    Ok(())
}

#[test]
fn prepared_report_rejects_changed_request_or_usage_identity() -> Result<(), HolonError> {
    let (_store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let selected =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let mut wrong = request(&caller, 2, 30);
    wrong.owner = VisualizerOwner::Visualizer(reference(&caller, 21));
    assert!(service
        .record(
            &space,
            &caller,
            wrong,
            reference(&caller, 20),
            selected.usage.clone(),
            VisualizerChoiceOrigin::Automatic,
            usage_report(&service, 1)
        )
        .is_err());
    let mut usage = selected.usage;
    usage.remove_related_holons("UsesVisualizer", vec![reference(&caller, 20)])?;
    usage.add_related_holons("UsesVisualizer", vec![reference(&caller, 21)])?;
    assert!(service
        .record(
            &space,
            &caller,
            request(&caller, 2, 30),
            reference(&caller, 20),
            usage,
            VisualizerChoiceOrigin::Automatic,
            usage_report(&service, 2)
        )
        .is_err());
    Ok(())
}

#[test]
fn queued_snapshot_owns_relationship_members_independently_of_live_edits() -> Result<(), HolonError>
{
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    store.block_commit.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let first = service.submit(&caller)?.unwrap();
    let other = space.get_transaction_manager().open_public_transaction(space.clone())?;
    service.begin(&space, &other)?;
    let mut live =
        service.select(&space, &other, request(&other, 2, 30), reference(&other, 21))?.usage;
    live.add_related_holons("PropertySalienceOverrides", vec![reference(&other, 20)])?;
    service.capture(&other, &live)?;
    let second = service.submit(&other)?.unwrap();
    live.remove_related_holons("PropertySalienceOverrides", vec![reference(&other, 20)])?;
    live.add_related_holons("PropertySalienceOverrides", vec![reference(&other, 21)])?;
    store.block_commit.store(false, Ordering::SeqCst);
    store.commit_changed.notify_all();
    wait_saved(&service, first);
    wait_saved(&service, second);
    let saved = service.find(&other, request(&other, 2, 30), reference(&other, 21))?.unwrap().usage;
    assert_eq!(
        dahn_selection::usage::related(&saved, "PropertySalienceOverrides")?,
        vec![reference(&other, 20)]
    );
    Ok(())
}

#[test]
fn automatic_selection_prepares_usage_without_repeating_eligibility_assessment(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let selection = service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert_eq!(selection.selected, reference(&caller, 20));
    assert!(selection.usage.as_ref().unwrap().initialized);
    assert_eq!(
        store
            .relationship_reads
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, name)| name == "ImplementedBy")
            .count(),
        1,
        "Usage preparation must use the selection's resolved context rather than assess it again"
    );
    let id = service.submit(&caller)?.unwrap();
    wait_saved(&service, id);
    assert_eq!(store.commits.lock().unwrap().len(), 1);
    Ok(())
}

#[test]
fn automatic_selection_without_a_usage_round_remains_read_only() -> Result<(), HolonError> {
    let (store, _space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    let selection = service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert!(selection.usage.is_none());
    assert!(!store
        .relationship_reads
        .lock()
        .unwrap()
        .iter()
        .any(|(_, name)| name == "UsedByVisualizerUsage"));
    assert!(store.commits.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn reopening_same_context_reuses_authorized_selection_and_live_usage_without_backend_reads(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let first = service.select_visualizer(&caller, request(&caller, 2, 30))?;
    let live = first.usage.unwrap().usage;
    let first_batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, first_batch);
    service.begin(&space, &caller)?;
    store.relationship_reads.lock().unwrap().clear();
    store.anchor_reads.lock().unwrap().clear();
    let reopened = service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert_eq!(reopened.selected, first.selected);
    assert_eq!(reopened.usage.unwrap().usage, live);
    assert!(
        store.relationship_reads.lock().unwrap().is_empty(),
        "Reopen must consume its established binding rather than resolve selection and Usage again"
    );
    assert!(store.anchor_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn established_bindings_do_not_alias_request_dimensions_or_contexts() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    let mut variants = vec![request(&caller, 3, 30), request(&caller, 2, 31)];
    let mut theme = request(&caller, 2, 30);
    theme.theme = reference(&caller, 61);
    variants.push(theme);
    let mut owner = request(&caller, 2, 30);
    owner.owner = VisualizerOwner::Dancer(reference(&caller, 40));
    variants.push(owner);
    let mut kind = request(&caller, 2, 30);
    kind.requested_kind = VisualizerKind::PropertyMap;
    variants.push(kind);
    service.begin(&space, &caller)?;
    for variant in variants {
        store.relationship_reads.lock().unwrap().clear();
        let _ = service.select_visualizer(&caller, variant);
        assert!(
            !store.relationship_reads.lock().unwrap().is_empty(),
            "A different matching context must undergo authoritative resolution"
        );
    }
    let other = space.get_transaction_manager().open_public_transaction(space.clone())?;
    service.begin(&space, &other)?;
    store.relationship_reads.lock().unwrap().clear();
    service.select_visualizer(&other, request(&other, 2, 30))?;
    assert!(!store.relationship_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn semantic_refresh_and_disposal_retire_bindings() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    service.invalidate_bindings()?;
    store.edge(1, "HasApplicableVisualizer", &[21]);
    service.begin(&space, &caller)?;
    let refreshed = service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert_eq!(refreshed.selected, reference(&caller, 21));
    let live = refreshed.usage.unwrap().usage;
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    service.release_context(caller.tx_id())?;
    service.begin(&space, &caller)?;
    store.relationship_reads.lock().unwrap().clear();
    let renewed = service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert_ne!(renewed.usage.unwrap().usage, live);
    assert!(!store.relationship_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn missing_artifact_cannot_be_authorized_by_a_retained_binding() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    service.begin(&space, &caller)?;
    store.artifact_missing.store(true, Ordering::SeqCst);
    assert!(service.select_visualizer(&caller, request(&caller, 2, 30)).is_err());
    store.artifact_missing.store(false, Ordering::SeqCst);
    assert_eq!(
        service.select_visualizer(&caller, request(&caller, 2, 30))?.selected,
        reference(&caller, 20)
    );
    Ok(())
}

#[test]
fn published_explicit_choice_replaces_automatic_binding_and_delayed_report_cannot_undo_it(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let automatic = service.select_visualizer(&caller, request(&caller, 2, 30))?.usage.unwrap();
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    store.edge(1, "HasApplicableVisualizer", &[20, 21]);
    service.begin(&space, &caller)?;
    let chosen =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 21))?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 21),
        chosen.usage,
        VisualizerChoiceOrigin::Explicit,
        usage_report(&service, 2),
    )?;
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        automatic.usage,
        VisualizerChoiceOrigin::Explicit,
        usage_report(&service, 1),
    )?;
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    service.begin(&space, &caller)?;
    assert_eq!(
        service.select_visualizer(&caller, request(&caller, 2, 30))?.selected,
        reference(&caller, 21)
    );
    Ok(())
}

#[test]
fn semantic_change_suspends_reuse_and_retires_resolutions_completed_during_the_change(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    let change = service.begin_binding_change()?;
    store.relationship_reads.lock().unwrap().clear();
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert!(!store.relationship_reads.lock().unwrap().is_empty());
    drop(change);
    store.relationship_reads.lock().unwrap().clear();
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert!(!store.relationship_reads.lock().unwrap().is_empty());
    store.relationship_reads.lock().unwrap().clear();
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert!(store.relationship_reads.lock().unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn command_dispatch_reuses_bindings_across_transient_builders_and_retires_them_for_semantic_writes(
) -> Result<(), HolonError> {
    use map_commands_contract::{
        HolonAction, HolonCommand, MapCommand, MapResult, TransactionAction, TransactionCommand,
        WritableHolonAction,
    };
    let (store, space, _) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let session = Arc::new(crate::RuntimeSession::new(space, None));
    let tx = session.begin_transaction().await?;
    let caller = session.get_transaction(&tx)?;
    let runtime = crate::Runtime::new(session.clone());
    let execute = |action| {
        runtime.execute_command(
            MapCommand::Transaction(TransactionCommand { context: caller.clone(), action }),
            crate::ExecutionPolicy::default(),
        )
    };
    execute(TransactionAction::BeginVisualizerUsageBatch).await?;
    execute(TransactionAction::SelectVisualizer { request: request(&caller, 2, 30) }).await?;
    let MapResult::Value(BaseValue::StringValue(batch)) =
        execute(TransactionAction::SubmitVisualizerUsageBatch).await?
    else {
        panic!("accepted batch");
    };
    wait_saved(&session.usage_transactions, batch.0.parse().unwrap());
    execute(TransactionAction::BeginVisualizerUsageBatch).await?;
    // Invocation builders create transient data without changing saved selection inputs.
    execute(TransactionAction::NewHolon { key: Some("presentation-invocation".into()) }).await?;
    store.relationship_reads.lock().unwrap().clear();
    execute(TransactionAction::SelectVisualizer { request: request(&caller, 2, 30) }).await?;
    assert!(store.relationship_reads.lock().unwrap().is_empty());
    // Even a rejected semantic write invalidates conservatively through the real ingress.
    let result = runtime
        .execute_command(
            MapCommand::Holon(HolonCommand {
                context: caller.clone(),
                target: reference(&caller, 20),
                action: HolonAction::Write(WritableHolonAction::WithPropertyValue {
                    name: PropertyName("Key".into()),
                    value: BaseValue::StringValue("changed".into()),
                }),
            }),
            crate::ExecutionPolicy::default(),
        )
        .await;
    assert!(result.is_err(), "saved references are read-only");
    store.relationship_reads.lock().unwrap().clear();
    execute(TransactionAction::SelectVisualizer { request: request(&caller, 2, 30) }).await?;
    assert!(!store.relationship_reads.lock().unwrap().is_empty());
    execute(TransactionAction::InvalidateVisualizerBindings).await?;
    store.relationship_reads.lock().unwrap().clear();
    execute(TransactionAction::SelectVisualizer { request: request(&caller, 2, 30) }).await?;
    assert!(!store.relationship_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn repeated_composition_consumes_all_established_bindings_without_selection_or_usage_reads(
) -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    for subject in 300..323 {
        store.nodes.lock().unwrap().insert(subject, PropertyMap::new());
        store.edge(subject, "DescribedBy", &[1]);
    }
    let service = UsageTransactions::default();
    for round in 0..2 {
        service.begin(&space, &caller)?;
        store.relationship_reads.lock().unwrap().clear();
        store.anchor_reads.lock().unwrap().clear();
        for (index, subject) in (300..323).enumerate() {
            let selection = service.select_visualizer(&caller, request(&caller, subject, 30))?;
            service.record(
                &space,
                &caller,
                request(&caller, subject, 30),
                selection.selected,
                selection.usage.unwrap().usage,
                VisualizerChoiceOrigin::Automatic,
                usage_report(&service, (round * 23 + index + 1) as u64),
            )?;
        }
        if round == 1 {
            assert!(
                store.relationship_reads.lock().unwrap().is_empty(),
                "All 23 prepared selection/Usage bindings must avoid backend resolution on repeat"
            );
            assert!(store.anchor_reads.lock().unwrap().is_empty());
        }
        let batch = service.submit(&caller)?.unwrap();
        wait_saved(&service, batch);
    }
    assert_eq!(store.commits.lock().unwrap().len(), 1);
    Ok(())
}

#[test]
fn retained_binding_reopens_without_waiting_for_its_pending_usage_commit() -> Result<(), HolonError>
{
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    store.block_commit.store(true, Ordering::SeqCst);
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    let first = service.submit(&caller)?.unwrap();
    let entered = store.commit_gate.lock().unwrap();
    let (entered, timeout) = store
        .commit_changed
        .wait_timeout_while(entered, std::time::Duration::from_secs(2), |entered| !*entered)
        .unwrap();
    assert!(!timeout.timed_out());
    drop(entered);
    service.begin(&space, &caller)?;
    store.relationship_reads.lock().unwrap().clear();
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert!(store.relationship_reads.lock().unwrap().is_empty());
    let second = service.submit(&caller)?.unwrap();
    assert_eq!(service.status(second)?, crate::usage_persistence::SaveStatus::Queued);
    store.block_commit.store(false, Ordering::SeqCst);
    store.commit_changed.notify_all();
    wait_saved(&service, first);
    wait_saved(&service, second);
    Ok(())
}

#[test]
fn first_composition_resolves_shared_selection_metadata_once() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    for subject in 300..323 {
        store.nodes.lock().unwrap().insert(subject, PropertyMap::new());
        store.edge(subject, "DescribedBy", &[1]);
    }
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    for subject in 300..323 {
        service.select_visualizer(&caller, request(&caller, subject, 30))?;
    }
    let reads = store.relationship_reads.lock().unwrap();
    for name in [
        "HasSlot",
        "AcceptsVisualizerType",
        "ForMetaDesignSystem",
        "DefinesDesignToken",
        "ImplementedBy",
        "RequiresOperator",
        "OffersOperator",
        "ConsumesDesignToken",
    ] {
        assert_eq!(reads.iter().filter(|(_, relationship)| relationship == name).count(), 1,
            "The composition should resolve shared {name} metadata once, without sharing subject selection or Usage configuration");
    }
    // Selection reads applicability once; the one shared committed-Usage lookup reads it once.
    assert_eq!(reads.iter().filter(|(_, name)| name == "HasApplicableVisualizer").count(), 2);
    assert_eq!(*store.usage_lookups.lock().unwrap(), vec![[20, 1, 30]]);
    Ok(())
}

#[test]
fn composition_metadata_preserves_actual_subject_type_and_exact_usage_configuration(
) -> Result<(), HolonError> {
    for kind in [VisualizerKind::Node, VisualizerKind::Property, VisualizerKind::Value] {
        let (store, space, caller) = setup();
        let service = UsageTransactions::default();
        store.edge(1, "HasApplicableVisualizer", &[20]);
        store.edge(5, "HasApplicableVisualizer", &[20]);
        store.edge(2, "HasApplicableVisualizer", &[20]);
        store.edge(4, "HasApplicableVisualizer", &[20]);
        store.edge(2, "ValueType", &[1]);
        store.edge(4, "ValueType", &[5]);
        service.begin(&space, &caller)?;
        let mut first = request(&caller, 2, 30);
        first.requested_kind = kind;
        let first = service.select_visualizer(&caller, first)?;
        let mut second = request(&caller, 4, 30);
        second.requested_kind = kind;
        let second = service.select_visualizer(&caller, second)?;
        assert_eq!(first.selected, second.selected);
        let first_usage = first.usage.unwrap().usage;
        let second_usage = second.usage.unwrap().usage;
        assert_ne!(first_usage, second_usage);
        let (a, b) = if kind == VisualizerKind::Property { (2, 4) } else { (1, 5) };
        assert_eq!(
            dahn_selection::usage::single(&first_usage, "ForSubjectType")?,
            reference(&caller, a)
        );
        assert_eq!(
            dahn_selection::usage::single(&second_usage, "ForSubjectType")?,
            reference(&caller, b)
        );
    }
    Ok(())
}

#[test]
fn shared_metadata_does_not_share_a_winner_or_hide_ambiguity() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    store.edge(5, "HasApplicableVisualizer", &[21]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    assert_eq!(
        service.select_visualizer(&caller, request(&caller, 2, 30))?.selected,
        reference(&caller, 20)
    );
    assert_eq!(
        service.select_visualizer(&caller, request(&caller, 4, 30))?.selected,
        reference(&caller, 21)
    );
    store.nodes.lock().unwrap().insert(6, PropertyMap::new());
    store.nodes.lock().unwrap().insert(7, PropertyMap::new());
    store.edge(6, "DescribedBy", &[7]);
    store.edge(7, "HasApplicableVisualizer", &[20, 21]);
    assert!(matches!(
        service.select_visualizer(&caller, request(&caller, 6, 30)),
        Err(HolonError::MultipleRelatedHolons { .. })
    ));
    Ok(())
}

#[test]
fn shared_metadata_does_not_suppress_usage_ambiguity() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    for usage in [200, 201] {
        store.nodes.lock().unwrap().insert(usage, PropertyMap::new());
        store.edge(usage, "DescribedBy", &[100]);
        store.edge(usage, "UsesVisualizer", &[20]);
        store.edge(usage, "ForSubjectType", &[1]);
        store.edge(usage, "SelectedForSlot", &[30]);
    }
    // Model the invalidation performed by semantic writes through the real ingress.
    service.invalidate_bindings()?;
    assert!(
        matches!(service.select_visualizer(&caller, request(&caller, 3, 30)), Err(HolonError::InvalidState(reason)) if reason.contains("Multiple applicable VisualizerUsages"))
    );
    Ok(())
}

#[test]
fn shared_metadata_still_authorizes_each_owner_slot_and_theme_context() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    let mut invalid_owner = request(&caller, 3, 30);
    invalid_owner.owner = VisualizerOwner::Visualizer(reference(&caller, 21));
    assert!(service.select_visualizer(&caller, invalid_owner).is_err());
    let mut invalid_theme = request(&caller, 3, 30);
    invalid_theme.theme = reference(&caller, 61);
    assert!(service.select_visualizer(&caller, invalid_theme).is_err());
    store.edge(31, "AcceptsVisualizerType", &[]);
    assert!(service.select_visualizer(&caller, request(&caller, 3, 31)).is_err());
    Ok(())
}

#[test]
fn shared_assessment_rechecks_artifact_availability_and_observes_recovery() -> Result<(), HolonError>
{
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    store.artifact_missing.store(true, Ordering::SeqCst);
    assert!(service.select_visualizer(&caller, request(&caller, 3, 30)).is_err());
    store.artifact_missing.store(false, Ordering::SeqCst);
    assert_eq!(
        service.select_visualizer(&caller, request(&caller, 3, 30))?.selected,
        reference(&caller, 20)
    );
    Ok(())
}

#[test]
fn metadata_index_is_bound_to_context_identity_not_space_local_transaction_number(
) -> Result<(), HolonError> {
    let (store, _space, caller) = setup();
    let (other_store, _other_space, other) = setup();
    assert_eq!(caller.tx_id(), other.tx_id());
    store.edge(1, "HasApplicableVisualizer", &[20]);
    other_store.edge(1, "HasApplicableVisualizer", &[21]);
    let mut metadata = dahn_selection::PreparedSelectionMetadata::default();
    assert_eq!(
        dahn_selection::select_visualizer_with_metadata(
            &caller,
            request(&caller, 2, 30),
            &mut metadata
        )?
        .0
        .selected,
        reference(&caller, 20)
    );
    assert_eq!(
        dahn_selection::select_visualizer_with_metadata(
            &other,
            request(&other, 2, 30),
            &mut metadata
        )?
        .0
        .selected,
        reference(&other, 21)
    );
    Ok(())
}

#[test]
fn selection_metadata_is_refreshed_at_the_next_composition_boundary() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[20]);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    service.begin(&space, &caller)?;
    store.relationship_reads.lock().unwrap().clear();
    // This target has no established binding. Its round must resolve fresh metadata.
    service.select_visualizer(&caller, request(&caller, 3, 30))?;
    assert_eq!(
        store
            .relationship_reads
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, name)| name == "ForMetaDesignSystem")
            .count(),
        1
    );
    Ok(())
}

fn inherited_setup() -> (Arc<Store>, Arc<HolonSpaceManager>, Arc<TransactionContext>) {
    let (store, space, caller) = setup();
    store.edge(1, "HasApplicableVisualizer", &[]);
    store.edge(5, "HasApplicableVisualizer", &[]);
    store.edge(1, "Extends", &[6]);
    store.edge(5, "Extends", &[6]);
    store.edge(6, "HasApplicableVisualizer", &[20]);
    (store, space, caller)
}

fn saved_usage(store: &Store, n: u32, scope: u32, slot: Option<u32>) {
    store.property(n, "Key", BaseValue::StringValue(format!("saved-usage-{n}").into()));
    store.edge(n, "DescribedBy", &[100]);
    store.edge(n, "UsesVisualizer", &[20]);
    store.edge(n, "ForSubjectType", &[scope]);
    if let Some(slot) = slot {
        store.edge(n, "SelectedForSlot", &[slot]);
    }
}

#[test]
fn inherited_defaults_share_one_live_usage_across_sibling_types_and_persist_once(
) -> Result<(), HolonError> {
    let (store, space, caller) = inherited_setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let first = service.select_visualizer(&caller, request(&caller, 2, 30))?.usage.unwrap();
    let second = service.select_visualizer(&caller, request(&caller, 4, 30))?.usage.unwrap();
    assert!(first.initialized);
    assert!(!second.initialized);
    assert_eq!(first.usage, second.usage);
    assert_eq!(
        dahn_selection::usage::single(&first.usage, "ForSubjectType")?,
        reference(&caller, 6)
    );
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        first.usage.clone(),
        VisualizerChoiceOrigin::Automatic,
        usage_report(&service, 1),
    )?;
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    assert_eq!(store.commits.lock().unwrap().len(), 1);
    let usages = dahn_selection::usage::instances(&caller, "VisualizerUsage.HolonType")?;
    assert_eq!(usages.len(), 1);
    assert_eq!(dahn_selection::usage::single(&usages[0], "ForSubjectType")?, reference(&caller, 6));
    assert!(
        dahn_selection::usage::instances(&caller, "VisualizerSlotPreference.HolonType")?.is_empty()
    );
    assert_eq!(caller.lookup().staged_count()?, 0);
    let next = space.get_transaction_manager().open_public_transaction(space.clone())?;
    service.begin(&space, &next)?;
    let reopened = service.select_visualizer(&next, request(&next, 4, 30))?.usage.unwrap();
    assert!(!reopened.initialized);
    assert_eq!(
        dahn_selection::usage::single(&reopened.usage, "ForSubjectType")?,
        reference(&next, 6)
    );
    Ok(())
}

#[test]
fn guest_resolution_prefers_nearest_scope_and_does_not_skip_ambiguity() -> Result<(), HolonError> {
    let (store, _space, caller) = inherited_setup();
    saved_usage(&store, 200, 6, None);
    saved_usage(&store, 201, 1, None);
    let resolve = || {
        caller.resolve_visualizer_usage(
            &reference(&caller, 20),
            &reference(&caller, 1),
            &reference(&caller, 30),
        )
    };
    let found = resolve()?;
    assert_eq!(found.usage, Some(reference(&caller, 201)));
    assert_eq!(found.usage_type, Some(reference(&caller, 1)));
    assert_eq!(found.default_type, reference(&caller, 6));
    assert_eq!(found.search_path, vec![reference(&caller, 1), reference(&caller, 6)]);
    saved_usage(&store, 202, 1, None);
    assert!(matches!(resolve(), Err(HolonError::InvalidState(_))));
    store.edge(202, "SelectedForSlot", &[30]);
    assert_eq!(resolve()?.usage, Some(reference(&caller, 202)));
    Ok(())
}

#[test]
fn inherited_resolution_reads_usage_membership_once_and_ignores_other_visualizers(
) -> Result<(), HolonError> {
    let (store, _space, caller) = inherited_setup();
    saved_usage(&store, 200, 6, None);
    store.edge(201, "UsesVisualizer", &[21]); // deliberately malformed unrelated Usage
    let result = caller.resolve_visualizer_usage(
        &reference(&caller, 20),
        &reference(&caller, 1),
        &reference(&caller, 30),
    )?;
    assert_eq!(result.usage, Some(reference(&caller, 200)));
    assert_eq!(
        store
            .relationship_reads
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, name)| name == "UsedByVisualizerUsage")
            .count(),
        1
    );
    Ok(())
}

#[test]
fn explicit_specialization_copies_configuration_and_later_resolution_prefers_it(
) -> Result<(), HolonError> {
    let (store, space, caller) = inherited_setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let mut shared =
        service.select_visualizer(&caller, request(&caller, 2, 30))?.usage.unwrap().usage;
    shared.with_property_value("DisplayName", "shared-config")?;
    service.capture(&caller, &shared)?;
    let specialized = service.specialize(
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        shared.clone(),
    )?;
    assert!(specialized.initialized);
    assert_ne!(specialized.usage, shared);
    assert_eq!(
        specialized.usage.property_value("DisplayName")?,
        shared.property_value("DisplayName")?
    );
    assert_eq!(
        dahn_selection::usage::single(&specialized.usage, "ForSubjectType")?,
        reference(&caller, 1)
    );
    assert!(dahn_selection::usage::related(&specialized.usage, "SelectedForSlot")?.is_empty());
    let mut local = specialized.usage.clone();
    local.with_property_value("DisplayName", "subtype-config")?;
    service.capture(&caller, &local)?;
    assert_eq!(
        shared.property_value("DisplayName")?,
        Some(BaseValue::StringValue("shared-config".into()))
    );
    let again = service.select_visualizer(&caller, request(&caller, 2, 30))?.usage.unwrap();
    assert_eq!(again.usage, local);
    let sibling = service.select_visualizer(&caller, request(&caller, 4, 30))?.usage.unwrap();
    assert_eq!(sibling.usage, shared);
    // An explicit retained supertype selection remains valid despite the subtype override.
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        shared,
        VisualizerChoiceOrigin::Explicit,
        usage_report(&service, 2),
    )?;
    let batch = service.submit(&caller)?.unwrap();
    wait_saved(&service, batch);
    let preferred =
        dahn_selection::usage::instances(&caller, "VisualizerSlotPreference.HolonType")?;
    assert_eq!(preferred.len(), 1);
    assert_eq!(
        dahn_selection::usage::single(&preferred[0], "PreferenceForSubjectType")?,
        reference(&caller, 1)
    );
    let chosen = dahn_selection::usage::single(&preferred[0], "PreferredUsage")?;
    assert_eq!(dahn_selection::usage::single(&chosen, "ForSubjectType")?, reference(&caller, 6));
    let next = space.get_transaction_manager().open_public_transaction(space.clone())?;
    service.begin(&space, &next)?;
    assert_eq!(
        service
            .select_visualizer(&next, request(&next, 2, 30))?
            .usage
            .unwrap()
            .usage
            .property_value("DisplayName")?,
        Some(BaseValue::StringValue("subtype-config".into()))
    );
    assert_eq!(dahn_selection::usage::instances(&next, "VisualizerUsage.HolonType")?.len(), 2);
    assert_eq!(store.commits.lock().unwrap().len(), 1);
    Ok(())
}

#[test]
fn explicit_shared_configuration_edit_does_not_specialize_subject() -> Result<(), HolonError> {
    let (_store, space, caller) = inherited_setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let mut usage =
        service.select_visualizer(&caller, request(&caller, 2, 30))?.usage.unwrap().usage;
    usage.with_property_value("DisplayName", "updated-shared")?;
    service.capture(&caller, &usage)?;
    let sibling = service.select_visualizer(&caller, request(&caller, 4, 30))?.usage.unwrap();
    assert_eq!(sibling.usage, usage);
    assert_eq!(dahn_selection::usage::single(&usage, "ForSubjectType")?, reference(&caller, 6));
    Ok(())
}

#[test]
fn guest_resolution_rejects_invalid_lineage_and_missing_applicability() {
    for parents in [vec![1], vec![6, 7], vec![]] {
        let (store, _space, caller) = inherited_setup();
        store.edge(1, "Extends", &parents);
        assert!(caller
            .resolve_visualizer_usage(
                &reference(&caller, 20),
                &reference(&caller, 1),
                &reference(&caller, 30)
            )
            .is_err());
    }
}

#[test]
fn lazy_defaults_use_semantic_node_property_and_value_lineages() -> Result<(), HolonError> {
    for kind in [VisualizerKind::Node, VisualizerKind::Property, VisualizerKind::Value] {
        let (store, space, caller) = inherited_setup();
        store.edge(2, "Extends", &[6]);
        store.edge(4, "Extends", &[6]);
        store.edge(2, "ValueType", &[1]);
        store.edge(4, "ValueType", &[5]);
        let service = UsageTransactions::default();
        service.begin(&space, &caller)?;
        let mut a = request(&caller, 2, 30);
        a.requested_kind = kind;
        let mut b = request(&caller, 4, 30);
        b.requested_kind = kind;
        let first = service.select_visualizer(&caller, a)?.usage.unwrap();
        let second = service.select_visualizer(&caller, b)?.usage.unwrap();
        assert_eq!(
            first.usage, second.usage,
            "{kind:?} must share the directly applicable default"
        );
        assert_eq!(
            dahn_selection::usage::single(&first.usage, "ForSubjectType")?,
            reference(&caller, 6)
        );
    }
    Ok(())
}

#[test]
fn specialization_reuses_existing_actual_type_configuration_without_overwriting_it(
) -> Result<(), HolonError> {
    let (_store, space, caller) = inherited_setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let shared = service.select_visualizer(&caller, request(&caller, 2, 30))?.usage.unwrap().usage;
    let first = service.specialize(
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        shared.clone(),
    )?;
    let mut local = first.usage.clone();
    local.with_property_value("DisplayName", "keep-this-configuration")?;
    let repeated =
        service.specialize(&caller, request(&caller, 2, 30), reference(&caller, 20), shared)?;
    assert_eq!(repeated.usage, local);
    assert!(!repeated.initialized);
    assert_eq!(
        repeated.usage.property_value("DisplayName")?,
        Some(BaseValue::StringValue("keep-this-configuration".into()))
    );
    Ok(())
}

#[test]
fn guest_usage_adapter_returns_a_typed_read_only_projection_for_hits_and_misses(
) -> Result<(), HolonError> {
    use holons_core::dances::{
        DanceRequest, DanceType, RequestBody, ResponseBody, VisualizerUsageResolution,
    };
    let (store, _space, caller) = inherited_setup();
    let invoke = || {
        holons_core::dances::holon_dance_adapter::resolve_visualizer_usage_dance(
            &caller,
            DanceRequest::new(
                "resolve_visualizer_usage".into(),
                DanceType::Standalone,
                RequestBody::TargetHolons(
                    RelationshipName("UsageResolutionInputs".into()),
                    vec![reference(&caller, 20), reference(&caller, 1), reference(&caller, 30)],
                ),
            ),
        )
    };
    let ResponseBody::HolonReference(body) = invoke()? else { panic!("projection") };
    let missing = VisualizerUsageResolution::from_projection(&body)?;
    assert!(missing.usage.is_none());
    assert_eq!(missing.default_type, reference(&caller, 6));
    assert_eq!(dahn_selection::usage::single(&body, "DescribedBy")?, reference(&caller, 153));
    saved_usage(&store, 200, 6, None);
    let ResponseBody::HolonReference(body) = invoke()? else { panic!("projection") };
    let found = VisualizerUsageResolution::from_projection(&body)?;
    assert_eq!(found.usage, Some(reference(&caller, 200)));
    assert_eq!(found.usage_type, Some(reference(&caller, 6)));
    assert_eq!(caller.lookup().staged_count()?, 0);
    assert!(store.commits.lock().unwrap().is_empty());
    assert!(holons_core::dances::holon_dance_adapter::resolve_visualizer_usage_dance(
        &caller,
        DanceRequest::new(
            "resolve_visualizer_usage".into(),
            DanceType::Standalone,
            RequestBody::None
        )
    )
    .is_err());
    Ok(())
}

#[test]
fn explicit_supertype_usage_selection_bypasses_nearer_ambiguity_without_forking(
) -> Result<(), HolonError> {
    let (store, space, caller) = inherited_setup();
    saved_usage(&store, 200, 6, None);
    saved_usage(&store, 201, 1, None);
    saved_usage(&store, 202, 1, None);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    assert!(service.select_visualizer(&caller, request(&caller, 2, 30)).is_err());
    let chosen = service.select_existing(
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        reference(&caller, 200),
    )?;
    assert!(!chosen.initialized);
    assert!(matches!(chosen.usage, HolonReference::Transient(_)));
    assert_eq!(
        dahn_selection::usage::single(&chosen.usage, "ForSubjectType")?,
        reference(&caller, 6)
    );
    let repeated = service.select_existing(
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        reference(&caller, 200),
    )?;
    assert_eq!(repeated.usage, chosen.usage);
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 20),
        chosen.usage.clone(),
        VisualizerChoiceOrigin::Explicit,
        usage_report(&service, 1),
    )?;
    assert_eq!(
        service.select_visualizer(&caller, request(&caller, 2, 30))?.usage.unwrap().usage,
        chosen.usage
    );
    assert_eq!(caller.lookup().staged_count()?, 0);
    store.artifact_missing.store(true, Ordering::SeqCst);
    assert!(service
        .select_existing(
            &caller,
            request(&caller, 2, 30),
            reference(&caller, 20),
            reference(&caller, 200)
        )
        .is_err());
    Ok(())
}

fn presentation_setup() -> (Arc<Store>, Arc<HolonSpaceManager>, Arc<TransactionContext>) {
    let (store, space, caller) = setup();
    for visualizer in [20, 21, 22] {
        store.property(
            visualizer,
            "Key",
            BaseValue::StringValue(format!("node-{visualizer}").into()),
        );
    }
    store.edge(20, "DescribedBy", &[12]);
    store.edge(12, "Extends", &[10]);
    store.edge(22, "DescribedBy", &[11]);
    store.edge(11, "Extends", &[10]);
    store.edge(22, "ImplementedBy", &[70]);
    store.edge(1, "HasApplicableVisualizer", &[20, 22]);
    store.edge(30, "AcceptsVisualizerType", &[12]);
    store.edge(12, "HasSlot", &[32, 33]);
    store.edge(32, "AcceptsVisualizerType", &[11]);
    for (slot, activation) in [(32, "presentation"), (33, "demand")] {
        for (name, value) in [
            ("CompositionRole", format!("role-{slot}")),
            ("CompositionKind", "titleBar".into()),
            ("CompositionSubject", "subject".into()),
            ("CompositionActivation", activation.into()),
        ] {
            store.property(slot, name, BaseValue::StringValue(value.into()));
        }
    }
    (store, space, caller)
}

#[test]
fn host_prepares_only_requested_bindings_and_reuses_them() -> Result<(), HolonError> {
    let (store, space, caller) = presentation_setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let root = service.select_visualizer(&caller, request(&caller, 2, 30))?;
    assert_eq!(root.selected, reference(&caller, 20));
    assert!(matches!(root.usage.unwrap().usage, HolonReference::Transient(_)));
    // Selecting a Node must not prepare its child bindings in a separate guest Dance.
    // Even active child Slot contracts are read only when their component is requested.
    assert!(!store
        .relationship_reads
        .lock()
        .unwrap()
        .iter()
        .any(|(source, _)| [32, 33].contains(source)));
    let child_request = VisualizerSelectionRequest {
        subject: reference(&caller, 2),
        requested_kind: VisualizerKind::TitleBar,
        owner: VisualizerOwner::Visualizer(reference(&caller, 20)),
        slot: reference(&caller, 32),
        theme: reference(&caller, 60),
    };
    let child = service.select_visualizer(&caller, child_request.clone())?;
    assert_eq!(child.selected, reference(&caller, 22));
    assert_eq!(caller.lookup().staged_count()?, 0);
    assert!(store.commits.lock().unwrap().is_empty());
    // The inactive Slot deliberately has no accepted types. Visiting it would fail.
    assert!(!store
        .relationship_reads
        .lock()
        .unwrap()
        .iter()
        .any(|(source, name)| *source == 33 && name == "AcceptsVisualizerType"));
    store.relationship_reads.lock().unwrap().clear();
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    service.select_visualizer(&caller, child_request)?;
    assert!(store.relationship_reads.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn an_explicit_root_candidate_preserves_the_current_choice_until_publication(
) -> Result<(), HolonError> {
    let (store, space, caller) = presentation_setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    store.edge(30, "AcceptsVisualizerType", &[10]);
    // The same request can explicitly choose another viable implementation despite ambiguity.
    let selected = reference(&caller, 22);
    let usage = service.select(&space, &caller, request(&caller, 2, 30), selected.clone())?;
    assert_eq!(dahn_selection::usage::single(&usage.usage, "UsesVisualizer")?, selected);
    assert_eq!(
        service.select_visualizer(&caller, request(&caller, 2, 30))?.selected,
        reference(&caller, 20)
    );
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        selected.clone(),
        usage.usage,
        VisualizerChoiceOrigin::Explicit,
        map_commands_contract::VisualizerUseReport {
            session: service.report_session.clone(),
            occurrence_id: "replacement".into(),
            sequence: 1,
        },
    )?;
    assert_eq!(service.select_visualizer(&caller, request(&caller, 2, 30))?.selected, selected);
    service.select_visualizer(&caller, request(&caller, 2, 30))?;
    Ok(())
}

#[test]
fn committed_usage_memo_keeps_visualizer_type_and_slot_distinct() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    for (subject, slot, visualizer) in
        [(2, 30, 20), (4, 30, 20), (2, 31, 20), (2, 30, 21), (3, 30, 20)]
    {
        service.select(
            &space,
            &caller,
            request(&caller, subject, slot),
            reference(&caller, visualizer),
        )?;
    }
    assert_eq!(
        *store.usage_lookups.lock().unwrap(),
        vec![[20, 1, 30], [20, 5, 30], [20, 1, 31], [21, 1, 30]]
    );
    Ok(())
}

#[test]
fn committed_usage_memo_is_discarded_between_rounds_and_contexts() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    for _ in 0..2 {
        service.begin(&space, &caller)?;
        for subject in [2, 3] {
            service.select(
                &space,
                &caller,
                request(&caller, subject, 30),
                reference(&caller, 20),
            )?;
        }
        if let Some(batch) = service.submit(&caller)? {
            wait_saved(&service, batch);
        }
    }
    let other = space.get_transaction_manager().open_public_transaction(space.clone())?;
    service.begin(&space, &other)?;
    service.select(&space, &other, request(&other, 2, 30), reference(&other, 20))?;
    assert_eq!(*store.usage_lookups.lock().unwrap(), vec![[20, 1, 30]; 3]);
    Ok(())
}

#[test]
fn committed_usage_memo_refreshes_after_semantic_invalidation() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let lookup =
        || service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20));
    lookup()?;
    lookup()?;
    service.invalidate_bindings()?;
    lookup()?;
    {
        let _change = service.begin_binding_change()?;
    }
    lookup()?;
    assert_eq!(*store.usage_lookups.lock().unwrap(), vec![[20, 1, 30]; 3]);
    Ok(())
}

#[test]
fn committed_usage_memo_does_not_cache_ambiguity_errors() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    saved_usage(&store, 200, 1, None);
    saved_usage(&store, 201, 1, None);
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let lookup =
        || service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20));
    assert!(lookup().is_err());
    store.edge(200, "SelectedForSlot", &[30]);
    lookup()?;
    lookup()?;
    assert_eq!(*store.usage_lookups.lock().unwrap(), vec![[20, 1, 30]; 2]);
    Ok(())
}

#[test]
fn committed_usage_memo_preserves_current_live_configuration() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let mut first =
        service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    assert!(first.initialized);
    first.usage.with_property_value("DisplayName", "edited-in-this-round")?;
    let second =
        service.select(&space, &caller, request(&caller, 3, 30), reference(&caller, 20))?;
    assert!(!second.initialized);
    assert_eq!(first.usage, second.usage);
    assert_eq!(
        second.usage.property_value("DisplayName")?,
        Some(BaseValue::StringValue("edited-in-this-round".into()))
    );
    assert_eq!(*store.usage_lookups.lock().unwrap(), vec![[20, 1, 30]]);
    Ok(())
}

#[test]
fn committed_usage_memo_preserves_slot_specific_saved_configuration() -> Result<(), HolonError> {
    let (store, space, caller) = setup();
    saved_usage(&store, 200, 1, Some(30));
    saved_usage(&store, 201, 1, Some(31));
    let service = UsageTransactions::default();
    service.begin(&space, &caller)?;
    let first = service.select(&space, &caller, request(&caller, 2, 30), reference(&caller, 20))?;
    let other = service.select(&space, &caller, request(&caller, 2, 31), reference(&caller, 20))?;
    let repeated =
        service.select(&space, &caller, request(&caller, 3, 30), reference(&caller, 20))?;
    assert_eq!(first.usage, repeated.usage);
    assert_ne!(first.usage, other.usage);
    assert_eq!(*store.usage_lookups.lock().unwrap(), vec![[20, 1, 30], [20, 1, 31]]);
    Ok(())
}

#[test]
fn alternate_visualizer_on_committed_result_keeps_usage_persistence_independent(
) -> Result<(), HolonError> {
    for saved_configuration in [false, true] {
        let (store, space, caller) = setup();
        if saved_configuration {
            saved_usage(&store, 200, 1, None);
            store.edge(200, "UsesVisualizer", &[21]);
        }
        let mut result = caller.mutation().new_holon(Some("completed-load-result".into()))?;
        result.with_descriptor(reference(&caller, 1))?;
        caller.commit()?;
        assert!(!caller.is_open());
        let mut selection_request = request(&caller, 2, 30);
        selection_request.subject = result.into();
        let service = UsageTransactions::default();
        service.begin(&space, &caller)?;
        let selected = reference(&caller, 21);
        let first = service.select(&space, &caller, selection_request.clone(), selected.clone())?;
        assert!(matches!(first.usage, HolonReference::Transient(_)));
        let repeated =
            service.select(&space, &caller, selection_request.clone(), selected.clone())?;
        assert_eq!(first.usage, repeated.usage);
        service.record(
            &space,
            &caller,
            selection_request,
            selected.clone(),
            first.usage,
            VisualizerChoiceOrigin::Explicit,
            usage_report(&service, 1),
        )?;
        let batch = service.submit(&caller)?.expect("choice must be persisted");
        wait_saved(&service, batch);
        assert!(!caller.is_open(), "choosing must not reopen the loader transaction");
        let commits = store.commits.lock().unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0], caller.tx_id().value());
        assert_ne!(
            commits[1],
            caller.tx_id().value(),
            "Usage commit must use an independent transaction"
        );
        drop(commits);
        let inspect = space.get_transaction_manager().open_public_transaction(space.clone())?;
        assert_eq!(
            dahn_selection::usage::instances(&inspect, "VisualizerUsage.HolonType")?.len(),
            1
        );
    }
    Ok(())
}
