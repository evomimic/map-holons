//! Canonical loading crosses the real host/guest boundary through the ordinary command runtime.
use core_types::{ContentSet, FileData};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::DanceInvocation;
use holons_core::{HolonReference, ReadableHolon, WritableHolon};
use holons_test::harness::helpers::init_probe_test_runtime;
use holons_test::DancesTestCase;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::sync::Arc;

async fn command(
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    action: TransactionAction,
) -> Result<MapResult, core_types::HolonError> {
    runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand { context: context.clone(), action }),
            ExecutionPolicy::default(),
        )
        .await
}

async fn prepare(
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    contents: &str,
) -> HolonReference {
    let result = command(
        runtime,
        context,
        TransactionAction::PrepareHolons {
            content_set: ContentSet {
                files_to_load: vec![FileData {
                    filename: "canonical.json".into(),
                    raw_contents: contents.into(),
                }],
            },
        },
    )
    .await
    .unwrap();
    let MapResult::Reference(reference) = result else { panic!("expected prepared reference") };
    reference
}

fn invocation(
    context: &Arc<TransactionContext>,
    request: HolonReference,
    affording: Option<HolonReference>,
) -> DanceInvocation {
    let descriptor =
        context.lookup().get_saved_holon_by_key(&"DanceInvocation.HolonType".into()).unwrap();
    let mut invocation =
        context.mutation().new_holon(Some("canonical-load-invocation".into())).unwrap();
    invocation.with_descriptor(descriptor.into()).unwrap();
    invocation.with_property_value("DanceName", "LoadHolons").unwrap();
    invocation.add_related_holons("Request", vec![request]).unwrap();
    if let Some(affording) = affording {
        invocation.add_related_holons("AffordingHolon", vec![affording]).unwrap();
    }
    DanceInvocation::new(invocation.into()).unwrap()
}

fn string(reference: &HolonReference, property: &str) -> String {
    match reference.property_value(property).unwrap().unwrap() {
        base_types::BaseValue::StringValue(value) => value.0,
        other => panic!("expected string: {other:?}"),
    }
}

async fn new_context(runtime: &Runtime) -> Arc<TransactionContext> {
    let tx_id = runtime.session().begin_transaction().await.unwrap();
    runtime.session().get_transaction(&tx_id).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn canonical_loader_preserves_authority_outcomes_and_isolation() {
    let (runtime, initial_tx, control) =
        init_probe_test_runtime(&mut DancesTestCase::default()).await;
    let unrelated = runtime.session().get_transaction(&initial_tx).unwrap();
    let source = unrelated.mutation().new_holon(Some("unrelated-edit".into())).unwrap();
    let unrelated_staged = unrelated.mutation().stage_new_holon(source).unwrap();

    // Collection implementations are supplied by the Space Navigator package, not Core Schema.
    let package_context = new_context(&runtime).await;
    let MapResult::Reference(package_response) = command(
        &runtime,
        &package_context,
        TransactionAction::LoadHolons {
            content_set: ContentSet {
                files_to_load: vec![FileData {
                    filename: "space-navigator/schema.json".into(),
                    raw_contents: include_str!(
                        "../../../generated/house-troupe/space-navigator/imports/schema.json"
                    )
                    .into(),
                }],
            },
        },
    )
    .await
    .unwrap() else {
        panic!("package response")
    };
    assert_eq!(string(&package_response, "LoadCommitStatus"), "Complete");
    command(&runtime, &package_context, TransactionAction::Dispose).await.unwrap();
    let context = new_context(&runtime).await;
    let saved = |key: &str| -> HolonReference {
        context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
    };
    let space = context.get_space_holon().unwrap().unwrap();
    let MapResult::VisualizerDiscovery(discovery) = command(
        &runtime,
        &context,
        TransactionAction::DiscoverVisualizers {
            request: map_commands_contract::VisualizerSelectionRequest {
                subject: space.clone(),
                requested_kind: map_commands_contract::VisualizerKind::Node,
                owner: map_commands_contract::VisualizerOwner::Visualizer(saved(
                    "PathInspector.RootedNavigationVisualizer",
                )),
                slot: saved("PathInspector.RootNodeSlot"),
                theme: saved("Demo1.DeepOceanTheme"),
            },
            current_selection: Some(saved("ConnectionsFirstInspector.NodeVisualizer")),
            retain_evidence: true,
        },
    )
    .await
    .unwrap() else {
        panic!("discovery evidence")
    };
    let destination = new_context(&runtime).await;
    let MapResult::Reference(projection) = command(
        &runtime,
        &destination,
        TransactionAction::ProjectVisualizerDiscovery { snapshot: discovery.snapshot.unwrap() },
    )
    .await
    .unwrap() else {
        panic!("discovery projection")
    };
    assert_eq!(string(&projection, "DiscoveryStopReason"), "holon_type_boundary");
    let explorer_request = || map_commands_contract::VisualizerSelectionRequest {
        subject: projection.clone(),
        requested_kind: map_commands_contract::VisualizerKind::Structure,
        owner: map_commands_contract::VisualizerOwner::Visualizer(saved(
            "VisualizerInspector.Visualizer",
        )),
        slot: saved("VisualizerInspector.DiscoveryExplorerSlot"),
        theme: saved("Demo1.DeepOceanTheme"),
    };
    let MapResult::VisualizerSelection(explorer) = command(
        &runtime,
        &destination,
        TransactionAction::SelectVisualizer { request: explorer_request() },
    )
    .await
    .unwrap() else {
        panic!("explorer selection")
    };
    assert_eq!(
        explorer.selected.holon_id().unwrap(),
        saved("DiscoveryTree.StructureVisualizer").holon_id().unwrap()
    );
    let MapResult::VisualizerUsageSelection(usage) = command(
        &runtime,
        &destination,
        TransactionAction::SelectVisualizerUsage {
            request: explorer_request(),
            selected: explorer.selected.clone(),
        },
    )
    .await
    .unwrap() else {
        panic!("explorer usage")
    };
    assert!(usage.initialized);
    let MapResult::VisualizerDiscovery(alternatives) = command(
        &runtime,
        &destination,
        TransactionAction::DiscoverVisualizers {
            request: explorer_request(),
            current_selection: Some(explorer.selected),
            retain_evidence: false,
        },
    )
    .await
    .unwrap() else {
        panic!("explorer alternatives")
    };
    for key in ["DiscoveryTree.StructureVisualizer", "DiscoveryLevels.StructureVisualizer"] {
        assert!(alternatives.candidates.iter().any(|candidate| candidate.visualizer == saved(key)
            && candidate.assessment == map_commands_contract::VisualizerAssessment::Viable));
        command(
            &runtime,
            &destination,
            TransactionAction::ChooseVisualizer {
                request: explorer_request(),
                candidate: saved(key),
            },
        )
        .await
        .unwrap();
    }
    command(&runtime, &destination, TransactionAction::Dispose).await.unwrap();
    command(&runtime, &context, TransactionAction::CheckLoadTarget { space: space.clone() })
        .await
        .unwrap();
    for (kind, subject, parent, slot, expected) in [
        (
            map_commands_contract::VisualizerKind::Node,
            space.clone(),
            "PathInspector.RootedNavigationVisualizer",
            "PathInspector.RootNodeSlot",
            "ConnectionsFirstInspector.NodeVisualizer",
        ),
        (
            map_commands_contract::VisualizerKind::ActionBar,
            space,
            "HolonInspector.NodeVisualizer",
            "HolonInspector.ActionsSlot",
            "GenericActions.ActionBarVisualizer",
        ),
        (
            map_commands_contract::VisualizerKind::Action,
            saved("LoadHolons.DanceType"),
            "GenericActions.ActionBarVisualizer",
            "GenericActions.ActionSlot",
            "LoadHolons.ActionVisualizer",
        ),
        (
            map_commands_contract::VisualizerKind::Action,
            saved("MaterializeVisualizer.DanceType"),
            "GenericActions.ActionBarVisualizer",
            "GenericActions.ActionSlot",
            "UnsupportedAction.ActionVisualizer",
        ),
    ] {
        let selection_request = || map_commands_contract::VisualizerSelectionRequest {
            subject: subject.clone(),
            requested_kind: kind,
            owner: map_commands_contract::VisualizerOwner::Visualizer(saved(parent)),
            theme: saved("Demo1.DeepOceanTheme"),
            slot: saved(slot),
        };
        let MapResult::VisualizerSelection(selection) = command(
            &runtime,
            &context,
            TransactionAction::SelectVisualizer { request: selection_request() },
        )
        .await
        .unwrap() else {
            panic!("selection")
        };
        assert_eq!(selection.selected.holon_id().unwrap(), saved(expected).holon_id().unwrap());
        let MapResult::VisualizerDiscovery(discovery) = command(
            &runtime,
            &context,
            TransactionAction::DiscoverVisualizers {
                request: selection_request(),
                current_selection: Some(selection.selected.clone()),
                retain_evidence: false,
            },
        )
        .await
        .unwrap() else {
            panic!("discovery")
        };
        assert_eq!(
            discovery.current_selection.unwrap().assessment,
            map_commands_contract::VisualizerAssessment::Viable
        );
        assert!(discovery
            .candidates
            .iter()
            .any(|candidate| candidate.visualizer == selection.selected
                && !candidate.declared_on.is_empty()));
        if kind == map_commands_contract::VisualizerKind::Node {
            for key in ["ConnectionsFirstInspector.NodeVisualizer", "HolonInspector.NodeVisualizer"]
            {
                assert!(discovery
                    .candidates
                    .iter()
                    .any(|candidate| candidate.visualizer == saved(key)
                        && candidate.assessment
                            == map_commands_contract::VisualizerAssessment::Viable));
                command(
                    &runtime,
                    &context,
                    TransactionAction::ChooseVisualizer {
                        request: selection_request(),
                        candidate: saved(key),
                    },
                )
                .await
                .unwrap();
                let mut invocation = context
                    .mutation()
                    .new_holon(Some("materialize-node-alternative".into()))
                    .unwrap();
                invocation.with_descriptor(saved("DanceInvocation.HolonType")).unwrap();
                invocation.with_property_value("DanceName", "MaterializeVisualizer").unwrap();
                invocation.add_related_holons("AffordingHolon", vec![saved(key)]).unwrap();
                let MapResult::Reference(response) = command(
                    &runtime,
                    &context,
                    TransactionAction::DanceV2 {
                        invocation: DanceInvocation::new(invocation.into()).unwrap(),
                    },
                )
                .await
                .unwrap() else {
                    panic!("materialized Node")
                };
                let bodies = response.related_holons("ResponseBody").unwrap();
                let projection = bodies.read().unwrap().get_members()[0].clone();
                let handle = string(&projection, "VisualizerArtifactHandle");
                assert!(matches!(
                    command(
                        &runtime,
                        &context,
                        TransactionAction::FetchArtifact { handle: handle.into() }
                    )
                    .await
                    .unwrap(),
                    MapResult::Value(base_types::BaseValue::BytesValue(_))
                ));
            }
        }
        let MapResult::VisualizerSelection(explicit) = command(
            &runtime,
            &context,
            TransactionAction::ChooseVisualizer {
                request: selection_request(),
                candidate: selection.selected.clone(),
            },
        )
        .await
        .unwrap() else {
            panic!("explicit choice")
        };
        assert_eq!(explicit.selected, selection.selected);

        // Usage initialization owns its commit, even when this caller has pending edits.
        let usage_context = new_context(&runtime).await;
        let edit =
            usage_context.mutation().new_holon(Some("pending-usage-caller-edit".into())).unwrap();
        usage_context.mutation().stage_new_holon(edit).unwrap();
        let MapResult::VisualizerUsageSelection(usage) = command(
            &runtime,
            &usage_context,
            TransactionAction::SelectVisualizerUsage {
                request: selection_request(),
                selected: selection.selected.clone(),
            },
        )
        .await
        .unwrap() else {
            panic!("usage selection")
        };
        assert!(usage.initialized);
        assert!(matches!(usage.usage, HolonReference::Smart(_)));
        assert!(usage_context.is_open());
        assert_eq!(usage_context.lookup().staged_count().unwrap(), 1);
        for name in [
            "PropertySalienceOverrides",
            "RelationshipSalienceOverrides",
            "ActionSalienceOverrides",
            "ActionGroupOverrides",
            "SelectedForSlot",
        ] {
            assert!(usage
                .usage
                .related_holons_with_hint(name, holons_core::RelationshipReadHint::RequireFresh)
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .is_empty());
        }
        command(
            &runtime,
            &usage_context,
            TransactionAction::RecordVisualizerUse {
                request: selection_request(),
                selected: selection.selected.clone(),
                usage: usage.usage.clone(),
                origin: map_commands_contract::VisualizerChoiceOrigin::Explicit,
                report: map_commands_contract::VisualizerUseReport {
                    session: usage.report_session.clone(),
                    occurrence_id: format!("canonical-{slot}"),
                    sequence: 1,
                },
            },
        )
        .await
        .unwrap();
        let slot = saved(slot);
        let selected_usages = slot
            .related_holons_with_hint(
                "HasSelectedUsage",
                holons_core::RelationshipReadHint::RequireFresh,
            )
            .unwrap();
        assert!(selected_usages.read().unwrap().get_members().contains(&usage.usage));
        let preferences = slot
            .related_holons_with_hint(
                "HasVisualizerPreference",
                holons_core::RelationshipReadHint::RequireFresh,
            )
            .unwrap();
        let preferences = preferences.read().unwrap().get_members().clone();
        assert!(preferences.iter().any(|preference| preference
            .related_holons("PreferredUsage")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .contains(&usage.usage)));
        let MapResult::VisualizerUsageSelection(reused) = command(
            &runtime,
            &usage_context,
            TransactionAction::SelectVisualizerUsage {
                request: selection_request(),
                selected: selection.selected.clone(),
            },
        )
        .await
        .unwrap() else {
            panic!("usage reuse")
        };
        assert!(!reused.initialized);
        assert_eq!(reused.usage, usage.usage);
        assert_eq!(usage_context.lookup().staged_count().unwrap(), 1);
        command(&runtime, &usage_context, TransactionAction::Dispose).await.unwrap();
    }
    // Assert the authored forward edge and its committed inverse at the consuming seam.
    let load_visualizer = saved("LoadHolons.ActionVisualizer");
    for slot_key in ["LoadHolons.DiagnosticsSlot", "LoadHolons.CommittedHolonsSlot"] {
        let slot = saved(slot_key);
        assert!(load_visualizer
            .related_holons("HasSlot")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .contains(&slot));
        assert!(slot
            .related_holons("SlotForVisualizer")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .contains(&load_visualizer));
        let MapResult::VisualizerSelection(selection) = command(
            &runtime,
            &context,
            TransactionAction::SelectCollectionVisualizer {
                collection: map_commands_contract::DescribedHolonCollection {
                    members: holons_core::HolonCollection::new_transient(),
                    element_type: saved(if slot_key == "LoadHolons.DiagnosticsSlot" {
                        "LoadDiagnostic.Projection"
                    } else {
                        "DanceImplementation.HolonType"
                    }),
                },
                parent_visualizer: load_visualizer.clone(),
                slot,
            },
        )
        .await
        .unwrap() else {
            panic!("collection selection")
        };
        assert_eq!(
            selection.selected.holon_id().unwrap(),
            saved("TableCollectionVisualizer.CollectionVisualizer").holon_id().unwrap()
        );
    }
    let load_dance = saved("LoadHolons.DanceType");
    assert!(load_visualizer
        .related_holons("ApplicableToType")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .contains(&load_dance));
    assert!(load_dance
        .related_holons("HasApplicableVisualizer")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .contains(&load_visualizer));
    let request = prepare(&runtime, &context, r#"{"holons":[]}"#).await;
    let missing = invocation(&context, request.clone(), None);
    let guest_error = context.initiate_invocation(missing.clone()).await.unwrap_err();
    assert!(
        matches!(guest_error, core_types::HolonError::MissingRequiredRelationship { .. }),
        "{guest_error:?}"
    );
    assert!(command(&runtime, &context, TransactionAction::DanceV2 { invocation: missing })
        .await
        .is_err());
    let invalid_subject: HolonReference =
        context.mutation().new_holon(Some("not-a-space".into())).unwrap().into();
    let invalid = invocation(&context, request.clone(), Some(invalid_subject));
    assert!(command(&runtime, &context, TransactionAction::DanceV2 { invocation: invalid })
        .await
        .is_err());
    let wrong_descriptor =
        context.lookup().get_saved_holon_by_key(&"DanceInvocation.HolonType".into()).unwrap();
    let mut wrong_request = context.mutation().new_holon(Some("wrong-request".into())).unwrap();
    wrong_request.with_descriptor(wrong_descriptor.into()).unwrap();
    let wrong = invocation(&context, wrong_request.into(), context.get_space_holon().unwrap());
    assert!(matches!(
        command(&runtime, &context, TransactionAction::DanceV2 { invocation: wrong }).await,
        Err(core_types::HolonError::WrongDescriptorKind { .. })
    ));
    assert!(context.staged_references().unwrap().is_empty());
    let space = context.get_space_holon().unwrap();
    let empty = invocation(&context, request, space);
    let MapResult::Reference(response) =
        command(&runtime, &context, TransactionAction::DanceV2 { invocation: empty.clone() })
            .await
            .unwrap()
    else {
        panic!("response")
    };
    assert_eq!(string(&response, "LoadCommitStatus"), "Skipped");
    assert!(context.is_open());
    assert!(command(&runtime, &context, TransactionAction::DanceV2 { invocation: empty })
        .await
        .is_err());
    command(&runtime, &context, TransactionAction::Dispose).await.unwrap();
    assert!(runtime.session().get_transaction(&context.tx_id()).is_err());

    for (contents, expected_status) in [
        (
            r#"{"holons":[{"key":"Unattached.HolonType","type":"MetaHolonType.MetaTypeDescriptor","properties":{"TypeName":"Unattached","TypeNamePlural":"UnattachedTypes","DisplayName":"Unattached","DisplayNamePlural":"Unattached types","Description":"Unattached finding fixture"},"relationships":[{"name":"Extends","target":{"$ref":"HolonType.TypeDescriptor"}},{"name":"ComponentOf","target":{"$ref":"MAP Core Schema-v0.0.7"}},{"name":"InstanceProperties","target":{"$ref":"DanceDescription.PropertyType"}}]}]}"#,
            "Rejected",
        ),
        (r#"{"holons":[{"key":"missing-target","type":"DoesNotExist.HolonType"}]}"#, "Skipped"),
        (
            r#"{"holons":[{"key":"Rejected.DanceImplementation","type":"DanceImplementation.HolonType","properties":{"ImplementationName":"Rejected","Undeclared":true},"relationships":[{"name":"ForDance","target":{"$ref":"LoadHolons.DanceType"}}]}]}"#,
            "Rejected",
        ),
        (
            r#"{"holons":[{"key":"Imported.DanceImplementation","type":"DanceImplementation.HolonType","properties":{"ImplementationName":"Imported"},"relationships":[{"name":"ForDance","target":{"$ref":"QueryDance.DanceType"}}]}]}"#,
            "Complete",
        ),
    ] {
        let context = new_context(&runtime).await;
        let request = prepare(&runtime, &context, contents).await;
        let invocation = invocation(&context, request, context.get_space_holon().unwrap());
        let result =
            command(&runtime, &context, TransactionAction::DanceV2 { invocation }).await.unwrap();
        let MapResult::Reference(response) = result else { panic!("expected response") };
        assert_eq!(string(&response, "LoadCommitStatus"), expected_status);
        assert_eq!(context.is_open(), expected_status != "Complete");
        let committed = assert_committed_review(&runtime, &context).await;
        assert_eq!(committed, if expected_status == "Complete" { 1 } else { 0 });

        if expected_status == "Rejected" {
            let commits = response
                .related_holons("LoadCommitResponse")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .clone();
            assert_eq!(commits.len(), 1);
            let commit = &commits[0];
            assert_eq!(
                commit.holon_descriptor().unwrap().header().type_name().unwrap().0,
                "CommitResponse"
            );
            let subjects = commit
                .related_holons("RejectedHolons")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .clone();
            let unattached = commit
                .related_holons("HasValidationFinding")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .clone();
            let mut finding_count = unattached.len();
            for subject in &subjects {
                let result = runtime
                    .execute_command(
                        MapCommand::Holon(map_commands_contract::HolonCommand {
                            context: context.clone(),
                            target: subject.clone(),
                            action: map_commands_contract::HolonAction::Read(
                                map_commands_contract::ReadableHolonAction::GetValidationFindings,
                            ),
                        }),
                        ExecutionPolicy::default(),
                    )
                    .await
                    .unwrap();
                let MapResult::ValidationFindings(findings) = result else {
                    panic!("validation findings")
                };
                assert!(!findings.is_empty());
                assert!(findings.iter().all(|finding| !finding.message.is_empty()));
                finding_count += findings.len();
            }
            assert_eq!(
                response.property_value("ValidationViolationCount").unwrap(),
                Some(base_types::BaseValue::IntegerValue(base_types::MapInteger(
                    finding_count as i64
                )))
            );
            let diagnostics = response
                .related_holons("HasDiagnostic")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .clone();
            assert_eq!(diagnostics.len(), finding_count);
            for diagnostic in diagnostics {
                assert!(matches!(diagnostic, HolonReference::Transient(_)));
                assert_eq!(
                    diagnostic.holon_descriptor().unwrap().header().type_name().unwrap().0,
                    "LoadDiagnostic"
                );
                assert!(diagnostic.property_value("Message").unwrap().is_some());
                let evidence = diagnostic
                    .related_holons("DiagnosticEvidence")
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_members()
                    .clone();
                assert_eq!(evidence.len(), 1);
                assert!(subjects.contains(&evidence[0]) || unattached.contains(&evidence[0]));
                let affected = diagnostic
                    .related_holons("DiagnosticSubject")
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_members()
                    .clone();
                if subjects.contains(&evidence[0]) {
                    assert_eq!(affected, evidence);
                    assert_eq!(
                        string(&diagnostic, "DiagnosticCategory"),
                        "Staged validation finding"
                    );
                } else {
                    assert!(affected.is_empty());
                    assert_eq!(
                        string(&diagnostic, "DiagnosticCategory"),
                        "Unattached validation finding"
                    );
                }
            }
            if contents.contains("Unattached.HolonType") {
                assert!(
                    !unattached.is_empty(),
                    "unstaged Schema findings must survive the loader response"
                );
                assert!(unattached
                    .iter()
                    .all(|finding| finding.property_value("Message").unwrap().is_some()));
            } else {
                let sources = response
                    .related_holons("HasValidationSource")
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_members()
                    .clone();
                assert_eq!(sources.len(), 1);
                assert_eq!(string(&sources[0], "Filename"), "canonical.json");
                assert!(sources[0]
                    .related_holons("ValidationSourceSubject")
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_members()
                    .contains(&subjects[0]));
            }
            assert!(
                matches!(response.property_value("ValidationViolationCount").unwrap(), Some(base_types::BaseValue::IntegerValue(value)) if value.0 > 0)
            );
            assert!(
                matches!(response.property_value("ErrorCount").unwrap(), Some(base_types::BaseValue::IntegerValue(value)) if value.0 == 0)
            );
        }
        if expected_status == "Skipped" {
            assert!(!response
                .related_holons("HasLoadError")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .is_empty());
        }
        assert_eq!(
            response.holon_descriptor().unwrap().header().type_name().unwrap().0,
            "HolonLoadResponse"
        );
        assert!(response
            .related_holons("ResponseBody")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .is_empty());
        assert!(!context.staged_references().unwrap().is_empty());
        command(&runtime, &context, TransactionAction::Dispose).await.unwrap();
        assert!(runtime.session().get_transaction(&context.tx_id()).is_err());
        if expected_status == "Complete" {
            let review = new_context(&runtime).await;
            let saved = review
                .lookup()
                .get_saved_holon_by_key(&"Imported.DanceImplementation".into())
                .unwrap();
            assert_eq!(string(&saved.into(), "ImplementationName"), "Imported");
            let descriptor =
                review.lookup().get_saved_holon_by_key(&"LoadHolons.DanceType".into()).unwrap();
            assert!(descriptor
                .related_holons("HasImplementation")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .iter()
                .any(|implementation| implementation
                    .key()
                    .unwrap()
                    .as_ref()
                    .map(|k| k.0.as_str())
                    == Some("LoadHolons.DanceImplementation")));
        }
    }
    // Use the existing isolated probe to force a real persistence failure after validation.
    // The loader still executes through production ingress and the ordinary controller.
    let schema_context = new_context(&runtime).await;
    let MapResult::Reference(schema_request) = command(
        &runtime,
        &schema_context,
        TransactionAction::PrepareHolons {
            content_set: holons_test::harness::helpers::build_book_person_inverse_content_set()
                .unwrap(),
        },
    )
    .await
    .unwrap() else {
        panic!("schema request")
    };
    let schema_invocation =
        invocation(&schema_context, schema_request, schema_context.get_space_holon().unwrap());
    let MapResult::Reference(schema_response) = command(
        &runtime,
        &schema_context,
        TransactionAction::DanceV2 { invocation: schema_invocation },
    )
    .await
    .unwrap() else {
        panic!("schema response")
    };
    assert_eq!(string(&schema_response, "LoadCommitStatus"), "Complete");
    let creation_context = new_context(&runtime).await;
    let book_request = prepare(&runtime, &creation_context, r#"{"holons":[{"key":"Book.CanonicalFailure","type":"Book.HolonType","properties":{"Title":"Book.CanonicalFailure"}}]}"#).await;
    let book_invocation =
        invocation(&creation_context, book_request, creation_context.get_space_holon().unwrap());
    let MapResult::Reference(book_response) = command(
        &runtime,
        &creation_context,
        TransactionAction::DanceV2 { invocation: book_invocation },
    )
    .await
    .unwrap() else {
        panic!("book response")
    };
    assert_eq!(string(&book_response, "LoadCommitStatus"), "Complete");
    let context = new_context(&runtime).await;
    let book = context.lookup().get_saved_holon_by_key(&"Book.CanonicalFailure".into()).unwrap();
    let title = context.lookup().get_saved_holon_by_key(&"Title.PropertyType".into()).unwrap();
    let source_id = book.holon_id().local_id().clone();
    let target_id = title.holon_id().local_id().clone();
    let mut staged = context.mutation().stage_new_version(book).unwrap();
    staged.add_related_holons("ReferencesProperty", vec![title.into()]).unwrap();
    let tag = core_types::encode_smartlink_tag(&core_types::SmartLinkTagInput {
        target_id: core_types::HolonId::Local(target_id.clone()),
        relationship_name: core_types::RelationshipName("ReferencesProperty".into()),
        canonical_key: core_types::CanonicalKey::new("stale-key").unwrap(),
        occurrence_id: None,
        relationship_property_values: Default::default(),
        target_property_cache_candidates: Vec::new(),
    })
    .unwrap();
    let planted = control.plant_stale_link(source_id, target_id, tag).await;
    let request = prepare(&runtime, &context, r#"{"holons":[{"key":"Partial.DanceImplementation","type":"DanceImplementation.HolonType","properties":{"ImplementationName":"Partial"},"relationships":[{"name":"ForDance","target":{"$ref":"QueryDance.DanceType"}}]}]}"#).await;
    let invocation = invocation(&context, request, context.get_space_holon().unwrap());
    let MapResult::Reference(response) =
        command(&runtime, &context, TransactionAction::DanceV2 { invocation }).await.unwrap()
    else {
        panic!("response")
    };
    assert_eq!(string(&response, "LoadCommitStatus"), "Incomplete");
    assert!(
        matches!(response.property_value("HolonsCommitted").unwrap(), Some(base_types::BaseValue::IntegerValue(value)) if value.0 > 0)
    );
    assert!(assert_committed_review(&runtime, &context).await > 0);
    assert!(context.is_open());
    assert!(!response
        .related_holons("HasLoadError")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .is_empty());
    assert!(!staged.commit_errors().unwrap().is_empty());
    control.delete_link(planted).await;

    assert!(unrelated.is_open());
    assert!(unrelated_staged.holon_id().is_err());
}

/// Read the exact Saved set in a fresh context, without carrying staged properties across.
async fn assert_committed_review(runtime: &Runtime, loader: &Arc<TransactionContext>) -> usize {
    let MapResult::Collection(members) =
        command(runtime, loader, TransactionAction::GetCommittedHolons).await.unwrap()
    else {
        panic!("committed collection")
    };
    let review = new_context(runtime).await;
    command(
        runtime,
        &review,
        TransactionAction::CheckLoadTarget {
            space: HolonReference::smart_from_id(
                review.space_read_handle(),
                loader.get_space_holon().unwrap().unwrap().holon_id().unwrap(),
            ),
        },
    )
    .await
    .unwrap();
    assert!(review.staged_references().unwrap().is_empty());
    for member in members.get_members() {
        let id = member.holon_id().unwrap();
        let saved = HolonReference::smart_from_id(review.space_read_handle(), id.clone());
        assert_eq!(saved.holon_id().unwrap(), id);
        assert!(saved.key().is_ok(), "saved key retrieval");
        assert!(!saved
            .related_holons("DescribedBy")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .is_empty());
    }
    let count = members.get_members().len();
    command(runtime, &review, TransactionAction::Dispose).await.unwrap();
    assert!(runtime.session().get_transaction(&review.tx_id()).is_err());
    count
}
