//! Canonical guest loading, consolidated transport, and read-only lifecycle evidence.
use async_trait::async_trait;
use core_types::{ContentSet, FileData, HolonError};
use holons_boundary::envelopes::{DanceRequestEnvelope, DanceResponseEnvelope};
use holons_core::dances::{DanceResponseReference, LoadedTheme};
use holons_core::{HolonReference, ReadableHolon, WritableHolon};
use holons_test::{
    init_runtime_with_initiator, init_tracing, setup_test_conductor, DancesTestCase,
    MockConductorConfig,
};
use holons_trust_channel::{DanceEnvelopeTransport, TrustChannel};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug)]
struct MeasuredTransport {
    backend: Arc<MockConductorConfig>,
    calls: Mutex<Vec<(String, u128, usize)>>,
}
#[async_trait]
impl DanceEnvelopeTransport for MeasuredTransport {
    async fn initiate_dance_envelope(
        &self,
        envelope: DanceRequestEnvelope,
    ) -> Result<DanceResponseEnvelope, HolonError> {
        let label = envelope.request.transport_label().to_owned();
        let start = Instant::now();
        let response = self.backend.initiate_dance_envelope(envelope).await?;
        self.calls.lock().unwrap().push((
            label,
            start.elapsed().as_micros(),
            serde_json::to_vec(&response).unwrap().len(),
        ));
        Ok(response)
    }
}
async fn begin(
    runtime: &Runtime,
) -> Arc<holons_core::core_shared_objects::transactions::TransactionContext> {
    let id = runtime.session().begin_transaction().await.unwrap();
    runtime.session().get_transaction(&id).unwrap()
}
async fn command(
    runtime: &Runtime,
    context: &Arc<holons_core::core_shared_objects::transactions::TransactionContext>,
    action: TransactionAction,
) -> MapResult {
    runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand { context: context.clone(), action }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
}
#[tokio::test(flavor = "multi_thread")]
async fn load_theme_is_theme_afforded_guest_resolved_and_constant_in_boundary_calls() {
    init_tracing();
    let backend = setup_test_conductor().await;
    let measured =
        Arc::new(MeasuredTransport { backend: backend.clone(), calls: Mutex::new(Vec::new()) });
    let (runtime, _) = init_runtime_with_initiator(
        &mut DancesTestCase::default(),
        Arc::new(TrustChannel::new(measured.clone())),
    )
    .await;
    let context = begin(&runtime).await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("generated/visualizer-commons/default-presentation/imports/schema.json");
    command(
        &runtime,
        &context,
        TransactionAction::LoadHolons {
            content_set: ContentSet {
                files_to_load: vec![FileData {
                    filename: path.display().to_string(),
                    raw_contents: std::fs::read_to_string(path).unwrap(),
                }],
            },
        },
    )
    .await;
    let context = begin(&runtime).await;
    let theme: HolonReference =
        context.lookup().get_saved_holon_by_key(&"DAHN.DefaultTheme".into()).unwrap().into();
    let theme_type: HolonReference =
        context.lookup().get_saved_holon_by_key(&"Theme.HolonType".into()).unwrap().into();
    let dance: HolonReference =
        context.lookup().get_saved_holon_by_key(&"LoadTheme.DanceType".into()).unwrap().into();
    assert!(theme_type
        .related_holons("AffordsDance")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .contains(&dance));
    assert!(theme.effective_descriptor().unwrap().affords_dance("LoadTheme").is_ok());
    let mut draft = context.mutation().new_holon(Some("LoadTheme.UnrelatedDraft".into())).unwrap();
    draft.with_property_value("Description", "unsaved subject edit").unwrap();
    let draft = context.mutation().stage_new_holon(draft).unwrap();
    let draft_reference = draft.clone();
    let mut expected_bindings = None;
    let mut large_warm_calls = Vec::new();
    for sample in ["cold", "warm"] {
        measured.calls.lock().unwrap().clear();
        let before = context.lookup().transient_count().unwrap();
        let started = Instant::now();
        let MapResult::LoadedTheme(loaded) =
            command(&runtime, &context, TransactionAction::LoadTheme { theme: theme.clone() })
                .await
        else {
            panic!("complete projection required")
        };
        let elapsed = started.elapsed();
        let calls = measured.calls.lock().unwrap().clone();
        assert_eq!(
            calls.iter().filter(|(label, _, _)| label == "canonical").count(),
            1,
            "{calls:?}"
        );
        assert!(!calls.iter().any(|(label, _, _)| label == "commit" || label == "load_holons"));
        assert_eq!(loaded.theme, theme);
        assert_eq!(loaded.tokens.len(), loaded.data.bindings.len());
        assert!(loaded.tokens.len() >= 63);
        assert_eq!(loaded.data.theme_key, "DAHN.DefaultTheme");
        if let Some(expected) = &expected_bindings {
            assert_eq!(&loaded.data, expected);
        }
        expected_bindings = Some(loaded.data.clone());
        if sample == "warm" {
            large_warm_calls = calls.iter().map(|(label, _, _)| label.clone()).collect();
        }
        assert!(
            context.lookup().transient_count().unwrap() - before < 10,
            "must not allocate one response holon per token"
        );
        assert_eq!(context.lookup().staged_count().unwrap(), 1);
        assert!(context.is_open());
        println!("[LoadTheme evidence] sample={sample} elapsed_us={} bindings={} scalar_bytes={} boundary_calls={calls:?}", elapsed.as_micros(), loaded.tokens.len(), serde_json::to_vec(&loaded.data).unwrap().len());
        // Exact token-version references survive guest projection and transport.
        for (token, binding) in loaded.tokens.iter().zip(&loaded.data.bindings) {
            assert_eq!(token.versioned_key().unwrap().0, binding.token_versioned_key);
        }
    }
    // The ordinary public canonical invocation exposes the same typed response.
    let invocation =
        holons_core::dances::implementations::load_theme::invocation(&context, theme.clone())
            .unwrap();
    let MapResult::Reference(response) =
        command(&runtime, &context, TransactionAction::DanceV2 { invocation }).await
    else {
        panic!("canonical response")
    };
    let body = DanceResponseReference::new(response).unwrap().require_response_body().unwrap();
    let count_before = measured.calls.lock().unwrap().len();
    let projection_started = Instant::now();
    let projected = LoadedTheme::from_projection(&body).unwrap();
    println!(
        "[LoadTheme evidence] local_response_projection_us={}",
        projection_started.elapsed().as_micros()
    );
    let token_wires =
        projected.tokens.iter().map(holons_boundary::HolonReferenceWire::from).collect::<Vec<_>>();
    let sdk_shape = serde_json::json!({ "theme": holons_boundary::HolonReferenceWire::from(&projected.theme),
        "meta_design_system": holons_boundary::HolonReferenceWire::from(&projected.meta_design_system),
        "tokens": token_wires, "data": projected.data });
    println!(
        "[LoadTheme evidence] consolidated_sdk_payload_bytes={}",
        serde_json::to_vec(&sdk_shape).unwrap().len()
    );
    assert_eq!(projected.data, expected_bindings.unwrap());
    assert_eq!(
        measured.calls.lock().unwrap().len(),
        count_before,
        "projection must never fetch source holons"
    );
    // An independent one-token vocabulary proves token count does not add host/guest calls.
    let small = serde_json::json!({ "holons": [
        { "key": "LoadTheme.SmallMDS", "type": "MetaDesignSystem.HolonType", "properties": { "MetaDesignSystemName": "LoadTheme.SmallMDS", "MetaDesignSystemDescription": "Independent one-token vocabulary for boundary-count verification" },
          "relationships": [{ "name": "DefinesDesignToken", "target": [{ "$ref": "CanvasGap.DesignToken" }] }] },
        { "key": "LoadTheme.SmallTheme", "type": "Theme.HolonType", "properties": { "ThemeName": "LoadTheme.SmallTheme", "ThemeDescription": "Independent one-token Theme for boundary-count verification" },
          "relationships": [{ "name": "ForMetaDesignSystem", "target": [{ "$ref": "LoadTheme.SmallMDS" }] }, { "name": "HasThemeTokenAssignment", "target": [{ "$ref": "LoadTheme.SmallAssignment" }] }] },
        { "key": "LoadTheme.SmallAssignment", "type": "ThemeTokenAssignment.HolonType", "properties": { "PresentationValue": "0.75rem" },
          "relationships": [{ "name": "ForDesignToken", "target": [{ "$ref": "CanvasGap.DesignToken" }] }] }
    ] });
    let import = begin(&runtime).await;
    let imported = command(
        &runtime,
        &import,
        TransactionAction::LoadHolons {
            content_set: ContentSet {
                files_to_load: vec![FileData {
                    filename: "small-theme.json".into(),
                    raw_contents: small.to_string(),
                }],
            },
        },
    )
    .await;
    let MapResult::Reference(imported) = imported else { panic!("import response") };
    assert_eq!(
        imported.property_value("LoadCommitStatus").unwrap(),
        Some(base_types::BaseValue::StringValue("Complete".into())),
        "small Theme import: {:?}",
        imported.property_value("DanceSummary")
    );
    let small_theme: HolonReference =
        context.lookup().get_saved_holon_by_key(&"LoadTheme.SmallTheme".into()).unwrap().into();
    for sample in ["small-cold", "small-warm"] {
        measured.calls.lock().unwrap().clear();
        let start = Instant::now();
        let MapResult::LoadedTheme(small) = command(
            &runtime,
            &context,
            TransactionAction::LoadTheme { theme: small_theme.clone() },
        )
        .await
        else {
            panic!("small projection")
        };
        assert_eq!(small.tokens.len(), 1);
        assert_eq!(small.data.bindings[0].presentation_value, "0.75rem");
        let calls = measured.calls.lock().unwrap().clone();
        if sample == "small-warm" {
            assert_eq!(
                calls.iter().map(|(label, _, _)| label.clone()).collect::<Vec<_>>(),
                large_warm_calls
            );
        }
        assert_eq!(calls.iter().filter(|(label, _, _)| label == "canonical").count(), 1);
        println!("[LoadTheme evidence] sample={sample} elapsed_us={} bindings=1 boundary_calls={calls:?}", start.elapsed().as_micros());
    }
    assert!(
        runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::LoadTheme { theme: projected.meta_design_system }
                }),
                ExecutionPolicy::default()
            )
            .await
            .is_err(),
        "an MDS cannot afford LoadTheme"
    );
    assert_eq!(context.lookup().staged_count().unwrap(), 1);
    assert_eq!(
        draft.property_value("Description").unwrap(),
        Some(base_types::BaseValue::StringValue("unsaved subject edit".into()))
    );
    assert_eq!(draft, draft_reference);
    drop(draft);
    drop(draft_reference);
    drop(context);
    drop(runtime);
    drop(measured);
    backend.shutdown().await;
}
