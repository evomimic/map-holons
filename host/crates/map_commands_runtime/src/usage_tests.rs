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
        // Model the committed DescribedBy inverse used for bounded instance lookup.
        if name == "DescribedBy" {
            for descriptor in old {
                if let Some(instances) = edges.get_mut(&(descriptor, "Instances".into())) {
                    instances.retain(|source| *source != n);
                }
            }
            for descriptor in targets {
                let instances = edges.entry((*descriptor, "Instances".into())).or_default();
                if !instances.contains(&n) {
                    instances.push(n);
                }
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
        let n = match key.0.as_str() {
            "VisualizerUsage.HolonType" => 100,
            "VisualizerSlotPreference.HolonType" => 101,
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
            VisualizerChoiceOrigin::Explicit
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
            VisualizerChoiceOrigin::Explicit
        )
        .is_err());
    service.record(
        &space,
        &caller,
        request(&caller, 2, 30),
        reference(&caller, 21),
        second.usage.clone(),
        VisualizerChoiceOrigin::Explicit,
    )?;
    let preferences =
        dahn_selection::usage::instances(&caller, "VisualizerSlotPreference.HolonType")?;
    assert_eq!(preferences.len(), 1);
    assert_eq!(dahn_selection::usage::single(&preferences[0], "PreferredUsage")?, second.usage);
    Ok(())
}
