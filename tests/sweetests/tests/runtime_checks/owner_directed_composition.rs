//! Persisted semantic composition through the normal client command boundary.
use holons_core::agent_spaces::{stage_personal_agent, PersonalName};
use holons_core::dahn::composition::SelectionReason;
use holons_core::reference_layer::HolonSpaceBehavior;
use holons_core::{Descriptor, HolonReference, ReadableHolon};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};

pub async fn establish_personal_test_context(runtime: &Runtime) {
    let id = runtime.session().begin_transaction().await.unwrap();
    let context = runtime.session().get_transaction(&id).unwrap();
    stage_personal_agent(
        &context,
        PersonalName {
            last_name: "Composition tester".into(),
            first_name: None,
            middle_name: None,
        },
    )
    .unwrap();
    let result = context.commit().unwrap();
    assert_eq!(
        result.property_value("CommitRequestStatus").unwrap(),
        Some(base_types::BaseValue::StringValue("Complete".into()))
    );
    context.dispose().unwrap();
}

pub async fn owner_directed_bindings_and_personal_preference_roundtrip(runtime: &Runtime) {
    establish_personal_test_context(runtime).await;
    let id = runtime.session().begin_transaction().await.unwrap();
    let context = runtime.session().get_transaction(&id).unwrap();
    let saved = |key: &str| -> HolonReference {
        context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
    };
    // The same field descriptor has different contextual types in these holders.
    // Resolve both in one Session, then revisit the first to exercise retained recipes.
    for (key, expected_type, expected_visualizer) in [
        (
            "InstanceDeletionAllowed.PropertyType",
            "MapBooleanValueType.BooleanValueType",
            "BooleanValue.ValueVisualizer",
        ),
        (
            "HolonSpaceName.PropertyType",
            "MapStringValueType.StringValueType",
            "StringValue.ValueVisualizer",
        ),
        (
            "InstanceDeletionAllowed.PropertyType",
            "MapBooleanValueType.BooleanValueType",
            "BooleanValue.ValueVisualizer",
        ),
    ] {
        let holder = saved(key);
        let result = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::ResolveComposition {
                        request: holons_core::dahn::composition::CompositionRequest {
                            value_holder: None,
                            owner: saved("HolonInspector.NodeVisualizer"),
                            subject: holder.clone(),
                            theme: saved("Demo1.ForestSanctuaryTheme"),
                            demand_slot: None,
                            collection_source: None,
                        },
                    },
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap();
        let MapResult::ResolvedComposition(result) = result else { panic!("expected composition") };
        let value = result
            .data
            .bindings
            .iter()
            .find(|b| b.property_name.as_deref() == Some("DefaultValue") && b.role == "value")
            .expect("default value presentation");
        assert_eq!(
            result.references[value.subject_descriptor].key().unwrap().unwrap().0,
            expected_type
        );
        assert_eq!(
            result.references[value.selected].key().unwrap().unwrap().0,
            expected_visualizer
        );
        assert_eq!(result.references[value.value_holder.expect("preserved holder")], holder);
        assert!(
            result.data.contextual_holders.iter().any(|index| result.references[*index] == holder),
            "core typing dependencies must govern reuse"
        );
    }
    // A direct Value demand must also carry its holder through the guest request schema.
    let MapResult::ResolvedComposition(direct) = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::ResolveComposition {
                    request: holons_core::dahn::composition::CompositionRequest {
                        value_holder: Some(saved("InstanceDeletionAllowed.PropertyType")),
                        owner: saved("GenericProperty.PropertyVisualizer"),
                        subject: saved("DefaultValue.PropertyType"),
                        theme: saved("Demo1.ForestSanctuaryTheme"),
                        demand_slot: Some(saved("GenericProperty.ValueSlot")),
                        collection_source: None,
                    },
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("expected direct Value composition")
    };
    assert_eq!(
        direct.references[direct.data.bindings[0].subject_descriptor].key().unwrap().unwrap().0,
        "MapBooleanValueType.BooleanValueType"
    );
    let composition_started = std::time::Instant::now();
    let owner = saved("HolonInspector.NodeVisualizer");
    let subject = context.get_space_holon().unwrap().unwrap();
    let theme = saved("Demo1.DeepOceanTheme");
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::ResolveComposition {
            request: holons_core::dahn::composition::CompositionRequest {
                value_holder: None,
                owner,
                subject,
                theme,
                demand_slot: None,
                collection_source: None,
            },
        },
    });
    let MapResult::ResolvedComposition(composition) =
        runtime.execute_command(command, ExecutionPolicy::default()).await.unwrap()
    else {
        panic!("composition result")
    };
    let initial_composition_elapsed = composition_started.elapsed();
    let subject = context.get_space_holon().unwrap().unwrap();
    let expected = subject
        .holon_descriptor()
        .unwrap()
        .effective_descriptor()
        .unwrap()
        .instance_properties()
        .unwrap()
        .into_iter()
        .filter(|p| !p.value_type().unwrap().is_array().unwrap())
        .count();
    assert!(composition
        .data
        .bindings
        .iter()
        .all(|b| b.role != "collection" && b.role != "diagnostics"));
    assert_eq!(
        composition.data.bindings.iter().filter(|b| b.property_name.is_some()).count(),
        expected * 2
    );
    assert!(composition.data.bindings.iter().all(|b| b.usage.is_none()));
    assert!(composition
        .data
        .bindings
        .iter()
        .any(|b| b.role == "titleBar" && b.reason == SelectionReason::PrescribedDefault));
    assert!(composition.data.reusable_across_rounds);
    assert_eq!(
        composition.data.work.candidate_population_requests, 0,
        "the shipped Inspector has declared defaults for its presented children"
    );
    assert_eq!(
        context.lookup().staged_count().unwrap(),
        0,
        "selection does not initialize a Usage"
    );
    assert!(!composition.data.metadata.is_empty());
    eprintln!(
        "Holon Inspector initial composition: {:?}, elapsed={initial_composition_elapsed:?}",
        composition.data.work
    );
    // Collection implementation declarations are a package, not part of the core bootstrap.
    let filename = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../generated/house-troupe/space-navigator/imports/schema.json");
    let MapResult::Reference(loaded) = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::LoadHolons {
                    content_set: core_types::ContentSet {
                        files_to_load: vec![core_types::FileData {
                            filename: filename.display().to_string(),
                            raw_contents: std::fs::read_to_string(&filename).unwrap(),
                        }],
                    },
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("collection package load")
    };
    assert_eq!(
        loaded.property_value("LoadCommitStatus").unwrap(),
        Some(base_types::BaseValue::StringValue("Complete".into()))
    );
    eprintln!("Collection package loaded; resolving explicit demand");
    let id = runtime.session().begin_transaction().await.unwrap();
    let context = runtime.session().get_transaction(&id).unwrap();
    let saved = |key: &str| -> HolonReference {
        context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
    };
    // The same interpreter resolves explicit collection demand without reading members.
    let subject = context.get_space_holon().unwrap().unwrap();
    let sources = subject
        .holon_descriptor()
        .unwrap()
        .effective_descriptor()
        .unwrap()
        .available_relationships(&subject)
        .unwrap();
    let request = holons_core::dahn::composition::CompositionRequest {
        value_holder: None,
        owner: saved("HolonInspector.NodeVisualizer"),
        subject,
        theme: saved("Demo1.DeepOceanTheme"),
        demand_slot: Some(saved("HolonInspector.CollectionsSlot")),
        collection_source: Some(sources[0].holon().clone()),
    };
    let MapResult::ResolvedComposition(demand) = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::ResolveComposition { request },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("demand result")
    };
    assert_eq!(demand.data.bindings.len(), 1);
    assert_eq!(
        demand.data.bindings[0].subject_binding,
        holons_core::dahn::CompositionSubject::SelectedCollection
    );
    assert_eq!(context.lookup().staged_count().unwrap(), 0);
    context.dispose().unwrap();
    eprintln!("Explicit collection demand passed; testing personal preferences");
    date_preferences(runtime).await;
}

async fn date_preferences(runtime: &Runtime) {
    use holons_core::core_shared_objects::transactions::TransactionContext;
    use holons_core::WritableHolon;
    use std::sync::Arc;
    async fn context(runtime: &Runtime) -> Arc<TransactionContext> {
        let id = runtime.session().begin_transaction().await.unwrap();
        runtime.session().get_transaction(&id).unwrap()
    }
    fn saved(context: &Arc<TransactionContext>, key: &str) -> HolonReference {
        context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
    }
    fn commit(context: &Arc<TransactionContext>) {
        let result = context.commit().unwrap();
        assert_eq!(
            result.property_value("CommitRequestStatus").unwrap(),
            Some(base_types::BaseValue::StringValue("Complete".into())),
            "{:?}",
            result.property_value("DanceSummary")
        );
    }
    async fn resolve(
        runtime: &Runtime,
        context: &Arc<TransactionContext>,
        record: HolonReference,
        suffix: &str,
        reconsider: bool,
    ) -> holons_core::dahn::composition::ResolvedComposition {
        let request = holons_core::dahn::composition::CompositionRequest {
            value_holder: None,
            owner: saved(context, &format!("PreferenceTest{suffix}.PropertyMapVisualizer")),
            subject: record,
            theme: saved(context, "Demo1.DeepOceanTheme"),
            demand_slot: None,
            collection_source: None,
        };
        let command = MapCommand::Transaction(TransactionCommand {
            context: context.clone(),
            action: if reconsider {
                TransactionAction::ReconsiderComposition { request }
            } else {
                TransactionAction::ResolveComposition { request }
            },
        });
        let MapResult::ResolvedComposition(composition) =
            runtime.execute_command(command, ExecutionPolicy::default()).await.unwrap()
        else {
            panic!("composition result")
        };
        composition
    }
    fn date_binding(
        result: &holons_core::dahn::composition::ResolvedComposition,
    ) -> &holons_core::dahn::composition::ChildBinding {
        result
            .data
            .bindings
            .iter()
            .find(|b| b.property_name.as_deref() == Some("PreferenceTestDate"))
            .unwrap()
    }
    let ctx = context(runtime).await;
    let filename = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../generated/json-imports/test/visualizer-preferences.json");
    let result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: ctx.clone(),
                action: TransactionAction::LoadHolons {
                    content_set: core_types::ContentSet {
                        files_to_load: vec![core_types::FileData {
                            filename: filename.display().to_string(),
                            raw_contents: std::fs::read_to_string(&filename).unwrap(),
                        }],
                    },
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap();
    let MapResult::Reference(result) = result else { panic!("fixture load response") };
    assert_eq!(
        result.property_value("LoadCommitStatus").unwrap(),
        Some(base_types::BaseValue::StringValue("Complete".into())),
        "summary={:?}; findings={:?}",
        result.property_value("DanceSummary"),
        ctx.staged_references()
            .unwrap()
            .iter()
            .map(|candidate| (candidate.key(), candidate.validation_findings()))
            .collect::<Vec<_>>()
    );
    let ctx = context(runtime).await;
    let MapResult::ResolvedComposition(initial_inspector) = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: ctx.clone(),
                action: TransactionAction::ResolveComposition {
                    request: holons_core::dahn::composition::CompositionRequest {
                        value_holder: None,
                        owner: saved(&ctx, "PreferenceTestInitialInspector.NodeVisualizer"),
                        subject: ctx.get_space_holon().unwrap().unwrap(),
                        theme: saved(&ctx, "Demo1.DeepOceanTheme"),
                        demand_slot: None,
                        collection_source: None,
                    },
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("initial collection composition")
    };
    assert_eq!(
        initial_inspector
            .data
            .bindings
            .iter()
            .filter(
                |b| b.subject_binding == holons_core::dahn::CompositionSubject::SelectedCollection
            )
            .count(),
        1
    );
    assert_eq!(ctx.lookup().staged_count().unwrap(), 0);
    let mut record = ctx.mutation().new_holon(None).unwrap();
    record.with_descriptor(saved(&ctx, "PreferenceTestRecord.HolonType")).unwrap();
    // Deliberately leave Date unset: selection depends on its declared ValueType.
    let record = ctx.mutation().stage_new_holon(record).unwrap();
    commit(&ctx);
    let record_id = record.holon_id().unwrap();
    let ctx = context(runtime).await;
    let record = HolonReference::smart_from_id(ctx.space_read_handle(), record_id.clone());
    // Establish confirmed absence; saving a preference must not reconsider this scope.
    let initial_started = std::time::Instant::now();
    let initial = resolve(runtime, &ctx, record.clone(), "A", false).await;
    let initial_elapsed = initial_started.elapsed();
    assert_eq!(date_binding(&initial).reason, SelectionReason::PrescribedDefault);
    assert_eq!(
        initial
            .data
            .bindings
            .iter()
            .filter(|b| b
                .property_name
                .as_deref()
                .is_some_and(|n| n.starts_with("PreferenceTestDate")))
            .count(),
        18
    );
    assert!(initial.data.work.binding_reuses >= 17);
    assert_eq!(initial.data.work.personal_population_requests, 1);
    assert_eq!(
        initial.data.work.candidate_population_requests, 0,
        "prescriptions do not enumerate unused candidates"
    );
    assert!(initial
        .data
        .populations
        .iter()
        .any(|p| p.relationship == "HasPersonalVisualizerPreference" && p.members.is_empty()));
    eprintln!(
        "initial 18-field composition work: {:?}, elapsed={initial_elapsed:?}",
        initial.data.work
    );
    // A different occurrence of the same declared type reuses the whole hierarchy.
    let mut occurrence = ctx.mutation().new_holon(None).unwrap();
    occurrence.with_descriptor(saved(&ctx, "PreferenceTestRecord.HolonType")).unwrap();
    let occurrence: HolonReference = occurrence.into();
    let reused = resolve(runtime, &ctx, occurrence.clone(), "A", false).await;
    assert_eq!(reused.data.work.selections, 0);
    assert_eq!(reused.data.work.personal_population_requests, 0);
    assert!(reused
        .data
        .subject_properties
        .iter()
        .all(|p| reused.references[p.subject] == occurrence));
    assert_eq!(date_binding(&reused).configuration, date_binding(&initial).configuration);
    // Reset retires descriptor handles; recipes reconstruct exact selections in a new transaction.
    runtime.session().space_manager().reset_session().unwrap();
    let rebuilt = resolve(runtime, &ctx, record.clone(), "A", false).await;
    assert_eq!(rebuilt.data.work.selections, 0);
    assert_eq!(
        rebuilt.references[date_binding(&rebuilt).selected],
        initial.references[date_binding(&initial).selected]
    );
    assert_eq!(date_binding(&rebuilt).configuration, date_binding(&initial).configuration);
    let preferred = saved(&ctx, "PreferenceTestPreferred.ValueVisualizer");
    holons_core::dahn::preferences::remember(
        &ctx,
        saved(&ctx, "PreferenceTestDate.StringValueType"),
        None,
        preferred,
        None,
    )
    .unwrap();
    commit(&ctx);
    let ctx = context(runtime).await;
    let retained = resolve(
        runtime,
        &ctx,
        HolonReference::smart_from_id(ctx.space_read_handle(), record_id.clone()),
        "A",
        false,
    )
    .await;
    assert_eq!(date_binding(&retained).reason, SelectionReason::PrescribedDefault);
    assert_eq!(retained.data.work.selections, 0);
    assert_eq!(retained.data.work.personal_population_requests, 0);
    assert_eq!(retained.data.work.candidate_population_requests, 0);
    assert_eq!(
        retained.data.populations, initial.data.populations,
        "absence remains captured evidence"
    );
    ctx.dispose().unwrap();
    for suffix in ["A", "B"] {
        let ctx = context(runtime).await;
        let result = resolve(
            runtime,
            &ctx,
            HolonReference::smart_from_id(ctx.space_read_handle(), record_id.clone()),
            suffix,
            true,
        )
        .await;
        let binding = date_binding(&result);
        assert_eq!(
            result.references[binding.selected],
            saved(&ctx, "PreferenceTestPreferred.ValueVisualizer")
        );
        assert!(!result
            .data
            .populations
            .iter()
            .find(|p| p.relationship == "HasPersonalVisualizerPreference")
            .unwrap()
            .members
            .is_empty());
        assert!(binding.usage.is_none());
        assert_eq!(binding.reason, SelectionReason::PersonalPreference);
        ctx.dispose().unwrap();
    }
    let ctx = context(runtime).await;
    let mut usage = ctx.mutation().new_holon(None).unwrap();
    usage.with_descriptor(saved(&ctx, "PreferenceTestUsage.HolonType")).unwrap();
    usage.with_property_value("PreferenceTestDateFormat", "long").unwrap();
    usage
        .add_related_holons(
            "UsesVisualizer",
            vec![saved(&ctx, "PreferenceTestPreferred.ValueVisualizer")],
        )
        .unwrap();
    usage
        .add_related_holons(
            "ForSubjectType",
            vec![saved(&ctx, "PreferenceTestDate.StringValueType")],
        )
        .unwrap();
    usage
        .add_related_holons(
            "UsageForAgent",
            vec![holons_core::dahn::PersonalPreferences::for_context(&ctx).unwrap().agent.unwrap()],
        )
        .unwrap();
    let usage: HolonReference = ctx.mutation().stage_new_holon(usage).unwrap().into();
    holons_core::dahn::preferences::remember(
        &ctx,
        saved(&ctx, "PreferenceTestDate.StringValueType"),
        None,
        saved(&ctx, "PreferenceTestPreferred.ValueVisualizer"),
        Some(usage),
    )
    .unwrap();
    commit(&ctx);
    let ctx = context(runtime).await;
    let retained = resolve(
        runtime,
        &ctx,
        HolonReference::smart_from_id(ctx.space_read_handle(), record_id.clone()),
        "A",
        false,
    )
    .await;
    assert!(date_binding(&retained).usage.is_none(), "new Usage does not alter captured settings");
    assert_eq!(retained.data.work.selections, 0);
    ctx.dispose().unwrap();
    for suffix in ["A", "B"] {
        let ctx = context(runtime).await;
        let result = resolve(
            runtime,
            &ctx,
            HolonReference::smart_from_id(ctx.space_read_handle(), record_id.clone()),
            suffix,
            true,
        )
        .await;
        let binding = date_binding(&result);
        assert!(binding.usage.is_some());
        assert_eq!(
            binding.configuration.get("PreferenceTestDateFormat"),
            Some(&base_types::BaseValue::StringValue("long".into()))
        );
        ctx.dispose().unwrap();
    }
    let ctx = context(runtime).await;
    holons_core::dahn::preferences::remember(
        &ctx,
        saved(&ctx, "PreferenceTestDate.StringValueType"),
        Some(saved(&ctx, "PreferenceTestA.Slot")),
        saved(&ctx, "PreferenceTestOverride.ValueVisualizer"),
        None,
    )
    .unwrap();
    commit(&ctx);
    for (suffix, expected) in [("A", "Override"), ("B", "Preferred"), ("Range", "Range")] {
        let ctx = context(runtime).await;
        let result = resolve(
            runtime,
            &ctx,
            HolonReference::smart_from_id(ctx.space_read_handle(), record_id.clone()),
            suffix,
            true,
        )
        .await;
        assert_eq!(
            result.references[date_binding(&result).selected],
            saved(&ctx, &format!("PreferenceTest{expected}.ValueVisualizer"))
        );
        ctx.dispose().unwrap();
    }
    // Explicit conversion preserves Slot-specific intent, including a repeat conversion.
    let ctx = context(runtime).await;
    let agent = holons_core::dahn::PersonalPreferences::for_context(&ctx).unwrap().agent.unwrap();
    let personal = agent
        .related_holons("HasPersonalVisualizerPreference")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .clone();
    let general = personal
        .into_iter()
        .find(|p| {
            p.related_holons("PreferenceForSlot").unwrap().read().unwrap().get_members().is_empty()
        })
        .unwrap();
    let usage =
        general.related_holons("PreferredUsage").unwrap().read().unwrap().get_members()[0].clone();
    let mut old = ctx.mutation().new_holon(Some("original-slot-preference".into())).unwrap();
    old.with_descriptor(saved(&ctx, "VisualizerSlotPreference.HolonType")).unwrap();
    old.add_related_holons("PreferenceForSlot", vec![saved(&ctx, "PreferenceTestB.Slot")]).unwrap();
    old.add_related_holons(
        "PreferenceForSubjectType",
        vec![saved(&ctx, "PreferenceTestDate.StringValueType")],
    )
    .unwrap();
    old.add_related_holons("PreferredUsage", vec![usage]).unwrap();
    let old = ctx.mutation().stage_new_holon(old).unwrap();
    commit(&ctx);
    let old_id = old.holon_id().unwrap();
    let ctx = context(runtime).await;
    let converted = holons_core::dahn::preferences::convert_slot_preference(
        &ctx,
        &HolonReference::smart_from_id(ctx.space_read_handle(), old_id.clone()),
    )
    .unwrap();
    assert_eq!(
        converted.related_holons("PreferenceForSlot").unwrap().read().unwrap().get_members(),
        &vec![saved(&ctx, "PreferenceTestB.Slot")]
    );
    commit(&ctx);
    let converted_id = converted.holon_id().unwrap();
    let ctx = context(runtime).await;
    let repeated = holons_core::dahn::preferences::convert_slot_preference(
        &ctx,
        &HolonReference::smart_from_id(ctx.space_read_handle(), old_id),
    )
    .unwrap();
    assert_eq!(repeated.holon_id().unwrap(), converted_id);
    assert_eq!(ctx.lookup().staged_count().unwrap(), 0);
    ctx.dispose().unwrap();
    let reopened = holons_test::runtime_for_bootstrapped_space(
        runtime.session().space_manager().get_dance_initiator().unwrap(),
        runtime.session().space_manager().get_space_holon_id().unwrap().unwrap(),
    );
    let ctx = context(&reopened).await;
    let result = resolve(
        &reopened,
        &ctx,
        HolonReference::smart_from_id(ctx.space_read_handle(), record_id.clone()),
        "B",
        false,
    )
    .await;
    assert_eq!(
        result.references[date_binding(&result).selected],
        saved(&ctx, "PreferenceTestPreferred.ValueVisualizer")
    );
    assert_eq!(
        date_binding(&result).configuration.get("PreferenceTestDateFormat"),
        Some(&base_types::BaseValue::StringValue("long".into()))
    );
    assert_eq!(ctx.lookup().staged_count().unwrap(), 0);
    // Set one Date without changing its declared type; populated and unset fields select identically.
    let original = HolonReference::smart_from_id(ctx.space_read_handle(), record_id);
    let mut update =
        ctx.mutation().stage_new_version_from_id(original.holon_id().unwrap()).unwrap();
    update.with_property_value("PreferenceTestDate", "2030-01-01").unwrap();
    commit(&ctx);
    let populated_id = update.holon_id().unwrap();
    let ctx = context(&reopened).await;
    let result = resolve(
        &reopened,
        &ctx,
        HolonReference::smart_from_id(ctx.space_read_handle(), populated_id),
        "B",
        false,
    )
    .await;
    let choices = result
        .data
        .bindings
        .iter()
        .filter(|b| b.property_name.as_deref().is_some_and(|n| n.starts_with("PreferenceTestDate")))
        .map(|b| b.selected)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(choices.len(), 1);
    ctx.dispose().unwrap();
}

/// Count the actual transport boundary independently of shared selector counters.
pub async fn measure_owner_composition_boundary(
    runtime: &Runtime,
    backend: std::sync::Arc<holons_test::MockConductorConfig>,
) {
    use async_trait::async_trait;
    use core_types::HolonError;
    use holons_boundary::envelopes::{DanceRequestEnvelope, DanceResponseEnvelope};
    use holons_trust_channel::{DanceEnvelopeTransport, TrustChannel};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    #[derive(Debug)]
    struct MeasuredTransport {
        backend: Arc<holons_test::MockConductorConfig>,
        calls: Mutex<Vec<(String, u128, usize)>>,
    }
    #[async_trait]
    impl DanceEnvelopeTransport for MeasuredTransport {
        async fn initiate_dance_envelope(
            &self,
            envelope: DanceRequestEnvelope,
        ) -> Result<DanceResponseEnvelope, HolonError> {
            let label = envelope.request.transport_label().to_owned();
            let started = Instant::now();
            let response = self.backend.initiate_dance_envelope(envelope).await?;
            self.calls.lock().unwrap().push((
                label,
                started.elapsed().as_micros(),
                serde_json::to_vec(&response).unwrap().len(),
            ));
            Ok(response)
        }
    }
    let measured = Arc::new(MeasuredTransport { backend, calls: Mutex::new(Vec::new()) });
    let fresh = holons_test::runtime_for_bootstrapped_space(
        Arc::new(TrustChannel::new(measured.clone())),
        runtime.session().space_manager().get_space_holon_id().unwrap().unwrap(),
    );
    let id = fresh.session().begin_transaction().await.unwrap();
    let context = fresh.session().get_transaction(&id).unwrap();
    let saved = |key: &str| -> HolonReference {
        context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
    };
    let request = holons_core::dahn::composition::CompositionRequest {
        value_holder: None,
        owner: saved("HolonInspector.NodeVisualizer"),
        subject: context.get_space_holon().unwrap().unwrap(),
        theme: saved("Demo1.DeepOceanTheme"),
        demand_slot: None,
        collection_source: None,
    };
    for sample in ["cold-host", "warm-host"] {
        measured.calls.lock().unwrap().clear();
        let started = Instant::now();
        let MapResult::ResolvedComposition(result) = fresh
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::ResolveComposition { request: request.clone() },
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("composition")
        };
        let calls = measured.calls.lock().unwrap().clone();
        assert_eq!(
            calls.iter().filter(|(label, _, _)| label == "canonical").count(),
            usize::from(sample == "cold-host"),
            "{calls:?}"
        );
        assert!(!calls.iter().any(|(label, _, _)| label == "commit" || label == "load_holons"));
        assert!(!result.data.metadata.is_empty());
        assert_eq!(
            result.data.work.personal_population_requests,
            usize::from(sample == "cold-host")
        );
        if sample == "warm-host" {
            assert_eq!(result.data.work.selections, 0);
            assert_eq!(result.data.work.candidate_population_requests, 0);
            assert!(result.data.work.binding_reuses >= result.data.bindings.len());
        }
        assert_eq!(context.lookup().staged_count().unwrap(), 0);
        eprintln!("[Composition boundary evidence] sample={sample} elapsed_us={} children={} work={:?} scalar_bytes={} boundary_calls={calls:?}",started.elapsed().as_micros(),result.data.bindings.len(),result.data.work,serde_json::to_vec(&result.data).unwrap().len());
    }
    context.dispose().unwrap();
}

/// Publishing a successor cannot change an established answer or a version-bound preference.
/// Run after the measured workload so added graph versions do not alter the baseline fixture.
pub async fn retained_visualizer_successor_preserves_established_version(runtime: &Runtime) {
    use holons_core::core_shared_objects::transactions::TransactionContext;
    use holons_core::{
        dahn::composition::{CompositionRequest, ResolvedComposition},
        WritableHolon,
    };
    use std::sync::Arc;
    async fn open(runtime: &Runtime) -> Arc<TransactionContext> {
        let id = runtime.session().begin_transaction().await.unwrap();
        runtime.session().get_transaction(&id).unwrap()
    }
    fn saved(context: &Arc<TransactionContext>, key: &str) -> HolonReference {
        context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
    }
    async fn resolve(
        runtime: &Runtime,
        context: &Arc<TransactionContext>,
        suffix: &str,
        reconsider: bool,
    ) -> ResolvedComposition {
        let mut subject = context.mutation().new_holon(None).unwrap();
        subject.with_descriptor(saved(context, "PreferenceTestRecord.HolonType")).unwrap();
        let request = CompositionRequest {
            value_holder: None,
            owner: saved(context, &format!("PreferenceTest{suffix}.PropertyMapVisualizer")),
            subject: subject.into(),
            theme: saved(context, "Demo1.DeepOceanTheme"),
            demand_slot: None,
            collection_source: None,
        };
        let action = if reconsider {
            TransactionAction::ReconsiderComposition { request }
        } else {
            TransactionAction::ResolveComposition { request }
        };
        let MapResult::ResolvedComposition(result) = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand { context: context.clone(), action }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("composition")
        };
        result
    }
    fn selected(result: &ResolvedComposition) -> HolonReference {
        let binding = result
            .data
            .bindings
            .iter()
            .find(|b| b.role == "value" && b.property_name.as_deref() == Some("PreferenceTestDate"))
            .unwrap();
        result.references[binding.selected].clone()
    }
    let context = open(runtime).await;
    let before = resolve(runtime, &context, "B", false).await;
    let established = selected(&before).holon_id().unwrap();
    let mut successor = context.mutation().stage_new_version_from_id(established.clone()).unwrap();
    successor.with_property_value("DisplayName", "Explicitly adopted successor").unwrap();
    let response = context.commit().unwrap();
    assert_eq!(
        response.property_value("CommitRequestStatus").unwrap(),
        Some(base_types::BaseValue::StringValue("Complete".into()))
    );
    let successor_id = successor.holon_id().unwrap();
    assert_ne!(established, successor_id);
    let context = open(runtime).await;
    let retained = resolve(runtime, &context, "B", false).await;
    assert_eq!(selected(&retained).holon_id().unwrap(), established);
    assert_eq!(retained.data.work.selections, 0);
    // Reconsideration consults the preference's exact target; it is not implicit
    // adoption of whatever successor a base-key lookup now returns.
    let refreshed = resolve(runtime, &context, "B", true).await;
    assert_eq!(selected(&refreshed).holon_id().unwrap(), established);
    assert!(refreshed.data.work.selections > 0);
    assert_eq!(
        saved(&context, "PreferenceTestPreferred.ValueVisualizer").holon_id().unwrap(),
        successor_id
    );
    assert_eq!(context.lookup().staged_count().unwrap(), 0);
    context.dispose().unwrap();

    // Replace saved preferences both within one Visualizer lineage (B) and across
    // lineages (A). A fresh observer must see the replacement, not an extra target.
    for suffix in ["B", "A"] {
        let context = open(runtime).await;
        let retained_before = resolve(runtime, &context, suffix, false).await;
        let old_choice = selected(&retained_before).holon_id().unwrap();
        let slot = saved(&context, &format!("PreferenceTest{suffix}.Slot"));
        let agent =
            holons_core::dahn::PersonalPreferences::for_context(&context).unwrap().agent.unwrap();
        let preference = agent
            .related_holons("HasPersonalVisualizerPreference")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .iter()
            .find(|preference| {
                preference
                    .related_holons("PreferenceForSlot")
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_members()
                    .contains(&slot)
            })
            .unwrap()
            .clone();
        let preference_id = preference.holon_id().unwrap();
        let old_target =
            preference.related_holons("PreferredVisualizer").unwrap().read().unwrap().get_members()
                [0]
            .clone();
        let replacement = holons_core::dahn::preferences::remember(
            &context,
            saved(&context, "PreferenceTestDate.StringValueType"),
            Some(slot),
            HolonReference::smart_from_id(context.space_read_handle(), successor_id.clone()),
            None,
        )
        .unwrap();
        assert_eq!(
            replacement
                .related_holons("PreferredVisualizer")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .len(),
            1,
            "staging already replaces the target"
        );
        let response = context.commit().unwrap();
        assert_eq!(
            response.property_value("CommitRequestStatus").unwrap(),
            Some(base_types::BaseValue::StringValue("Complete".into()))
        );
        assert_eq!(
            replacement.holon_id().unwrap(),
            preference_id,
            "non-definitional replacement preserves the source version"
        );
        context.dispose().unwrap();
        runtime.session().space_manager().reset_session().unwrap();

        let context = open(runtime).await;
        let preference =
            HolonReference::smart_from_id(context.space_read_handle(), preference_id.clone());
        let targets = preference
            .related_holons("PreferredVisualizer")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .clone();
        assert_eq!(
            targets.iter().map(|target| target.holon_id().unwrap()).collect::<Vec<_>>(),
            vec![successor_id.clone()],
            "persisted preference must contain only the replacement"
        );
        let old_target = HolonReference::smart_from_id(
            context.space_read_handle(),
            old_target.holon_id().unwrap(),
        );
        assert!(
            !old_target
                .related_holons("ChosenByVisualizerPreference")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .iter()
                .any(|member| member.holon_id().unwrap() == preference_id),
            "the removed target must lose this preference's inverse membership"
        );
        let successor =
            HolonReference::smart_from_id(context.space_read_handle(), successor_id.clone());
        assert_eq!(
            successor
                .related_holons("ChosenByVisualizerPreference")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .iter()
                .filter(|member| member.holon_id().unwrap() == preference_id)
                .count(),
            1
        );
        let retained = resolve(runtime, &context, suffix, false).await;
        assert_eq!(selected(&retained).holon_id().unwrap(), old_choice);
        assert_eq!(retained.data.work.selections, 0);
        let adopted = resolve(runtime, &context, suffix, true).await;
        assert_eq!(selected(&adopted).holon_id().unwrap(), successor_id);
        assert!(adopted.data.work.selections > 0);
        context.dispose().unwrap();
    }

    // Clearing optional Usage is removal-only: there is no remaining member to
    // drive Pass 2. Keep the cross-cutting preference's exact Visualizer unchanged.
    let context = open(runtime).await;
    let agent =
        holons_core::dahn::PersonalPreferences::for_context(&context).unwrap().agent.unwrap();
    let preference = agent
        .related_holons("HasPersonalVisualizerPreference")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .iter()
        .find(|preference| {
            preference
                .related_holons("PreferenceForSlot")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .is_empty()
        })
        .unwrap()
        .clone();
    let preference_id = preference.holon_id().unwrap();
    let visualizer =
        preference.related_holons("PreferredVisualizer").unwrap().read().unwrap().get_members()[0]
            .clone();
    let usage = preference.related_holons("PreferredUsage").unwrap().read().unwrap().get_members()
        [0]
    .holon_id()
    .unwrap();
    holons_core::dahn::preferences::remember(
        &context,
        saved(&context, "PreferenceTestDate.StringValueType"),
        None,
        visualizer,
        None,
    )
    .unwrap();
    let response = context.commit().unwrap();
    assert_eq!(
        response.property_value("CommitRequestStatus").unwrap(),
        Some(base_types::BaseValue::StringValue("Complete".into()))
    );
    context.dispose().unwrap();
    runtime.session().space_manager().reset_session().unwrap();
    let context = open(runtime).await;
    let preference =
        HolonReference::smart_from_id(context.space_read_handle(), preference_id.clone());
    assert!(
        preference
            .related_holons("PreferredUsage")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .is_empty(),
        "removal-only updates persist an empty collection"
    );
    // The Usage remains a saved object; only the preference's membership was removed.
    let usage = HolonReference::smart_from_id(context.space_read_handle(), usage);
    assert!(!usage
        .related_holons("PreferredByVisualizerPreference")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .iter()
        .any(|member| member.holon_id().unwrap() == preference_id));
    assert_eq!(
        usage.property_value("PreferenceTestDateFormat").unwrap(),
        Some(base_types::BaseValue::StringValue("long".into()))
    );
    holons_core::dahn::PersonalPreferences::for_context(&context).unwrap();
    context.dispose().unwrap();
    preference_versions_share_inverse_until_last_removal(runtime, preference_id, successor_id)
        .await;
}

/// Different saved source versions can support one lineage-bound inverse. Removing
/// one forward edge must retain that inverse until its final supporting edge is gone.
async fn preference_versions_share_inverse_until_last_removal(
    runtime: &Runtime,
    preference_id: core_types::HolonId,
    replacement_id: core_types::HolonId,
) {
    use holons_core::WritableHolon;
    let id = runtime.session().begin_transaction().await.unwrap();
    let context = runtime.session().get_transaction(&id).unwrap();
    let root = HolonReference::smart_from_id(context.space_read_handle(), preference_id.clone());
    let old = root.related_holons("PreferredVisualizer").unwrap().read().unwrap().get_members()[0]
        .holon_id()
        .unwrap();
    let mut successor =
        context.mutation().stage_new_version_from_id(preference_id.clone()).unwrap();
    successor.with_property_value("Key", "Preference.RemovalHistory").unwrap();
    let response = context.commit().unwrap();
    assert_eq!(
        response.property_value("CommitRequestStatus").unwrap(),
        Some(base_types::BaseValue::StringValue("Complete".into()))
    );
    let successor_id = successor.holon_id().unwrap();
    assert_ne!(successor_id, preference_id);
    context.dispose().unwrap();

    // Update the successor's mutable membership, leaving the root's edge intact.
    for source in [successor_id.clone(), preference_id.clone()] {
        runtime.session().space_manager().reset_session().unwrap();
        let id = runtime.session().begin_transaction().await.unwrap();
        let context = runtime.session().get_transaction(&id).unwrap();
        let mut staged = context.mutation().stage_new_version_from_id(source.clone()).unwrap();
        staged
            .remove_related_holons(
                "PreferredVisualizer",
                vec![HolonReference::smart_from_id(context.space_read_handle(), old.clone())],
            )
            .unwrap();
        staged
            .add_related_holons(
                "PreferredVisualizer",
                vec![HolonReference::smart_from_id(
                    context.space_read_handle(),
                    replacement_id.clone(),
                )],
            )
            .unwrap();
        let response = context.commit().unwrap();
        assert_eq!(
            response.property_value("CommitRequestStatus").unwrap(),
            Some(base_types::BaseValue::StringValue("Complete".into()))
        );
        assert_eq!(staged.holon_id().unwrap(), source);
        context.dispose().unwrap();
        runtime.session().space_manager().reset_session().unwrap();
        let id = runtime.session().begin_transaction().await.unwrap();
        let context = runtime.session().get_transaction(&id).unwrap();
        for version in [&preference_id, &successor_id] {
            let reference =
                HolonReference::smart_from_id(context.space_read_handle(), version.clone());
            let expected = if version == &preference_id && source == successor_id {
                &old
            } else {
                &replacement_id
            };
            assert_eq!(
                reference
                    .related_holons("PreferredVisualizer")
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_members()
                    .iter()
                    .map(|member| member.holon_id().unwrap())
                    .collect::<Vec<_>>(),
                vec![expected.clone()]
            );
        }
        for target in [&old, &replacement_id] {
            let target_reference =
                HolonReference::smart_from_id(context.space_read_handle(), target.clone());
            let count = target_reference
                .related_holons("ChosenByVisualizerPreference")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .iter()
                .filter(|member| member.holon_id().unwrap() == preference_id)
                .count();
            assert_eq!(
                count,
                usize::from(target == &replacement_id || source == successor_id),
                "shared inverse reflects all supporting exact source versions"
            );
        }
        context.dispose().unwrap();
    }
}
