//! Opt-in call-site audit and independent source-chain evidence on an isolated conductor.
#[allow(dead_code)]
mod execution_steps;
#[allow(dead_code)]
mod fixture_cases {
    pub mod commit_scalar_constraints_fixture;
    pub mod described_instance_helpers;
    pub mod query_qry2_seed_expand_fixture;
}

use execution_steps::dance_test_runner::run_dance_test_case;
use fixture_cases::commit_scalar_constraints_fixture::EXPECTED_SCALAR_VIOLATIONS;
use holochain::prelude::Signal;
use holons_test::harness::helpers::{
    build_core_schema_bootstrap_content_set, init_runtime_with_initiator,
    setup_probe_enabled_conductor,
};
use holons_test::{DanceTestStep, ExpectedCommitStatus, TestExecutionState};
use holons_trust_channel::TrustChannel;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc::Receiver;

async fn audit(signals: &mut Receiver<serde_json::Value>) -> serde_json::Value {
    tokio::time::timeout(Duration::from_secs(30), signals.recv())
        .await
        .expect("requires a holons/commit-audit build; no Commit audit signal arrived")
        .expect("Commit signal collector must remain live")
}

/// Audits node create/update and SmartLink create attempts for a create-only rejection.
/// Delete attempts and LocalHolonSpace link attempts are outside this call-site audit.
/// Run after building with holons/commit-audit; ordinary artifacts have no audit signals.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires the opt-in holons/commit-audit diagnostic build"]
async fn scalar_create_rejection_and_clean_bootstrap() {
    let mut case =
        fixture_cases::commit_scalar_constraints_fixture::commit_scalar_constraints_fixture()
            .unwrap();
    let backend = setup_probe_enabled_conductor().await;
    // Signals are broadcast per installed app; this is the id the probe-enabled conductor uses.
    let mut source = backend.conductor.subscribe_to_app_signals("probe-enabled-app".into());
    let (audit_sender, mut signals) = tokio::sync::mpsc::channel(16);
    // Drain during bootstrap: the conductor's broadcast ring holds only 64 signals,
    // while bootstrap's post_commit notification burst is much larger.
    let collector = tokio::spawn(async move {
        loop {
            let signal = match source.recv().await {
                Ok(signal) => signal,
                // A lagged receiver stays usable; a dropped audit surfaces as a timeout below.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            };
            if let Signal::App { signal, .. } = signal {
                if let Ok(value) = signal.into_inner().decode::<serde_json::Value>() {
                    if value["kind"] == "CommitAudit" && audit_sender.send(value).await.is_err() {
                        return;
                    }
                }
            }
        }
    });
    let initiator = Arc::new(TrustChannel::new(backend.clone()));
    let (runtime, tx_id) = init_runtime_with_initiator(&mut case, initiator).await;
    let bootstrap = audit(&mut signals).await;
    assert_eq!(bootstrap["violations"], 0, "strict manifest-selected bootstrap");
    assert_eq!(bootstrap["property_conformance_dispatched"], true);
    assert!(bootstrap["reached_constraints"].as_u64().unwrap() > 0);
    assert!(bootstrap["evaluated_constraints"].as_u64().unwrap() > 0);
    // The selected corpus has only one executable configured scalar constraint.
    // Thus an actual scalar evaluation above proves that Length16k ran.
    let mut scalar_keys = Vec::new();
    for file in build_core_schema_bootstrap_content_set().unwrap().files_to_load {
        let document: serde_json::Value = serde_json::from_str(&file.raw_contents).unwrap();
        for holon in document["holons"].as_array().unwrap() {
            if matches!(
                holon["type"].as_str(),
                Some(
                    "StringLengthConstraint.ConstraintType"
                        | "BytesLengthConstraint.ConstraintType"
                        | "NumericRangeConstraint.ConstraintType"
                )
            ) {
                scalar_keys.push(holon["key"].as_str().unwrap().to_owned());
            }
        }
    }
    assert_eq!(scalar_keys, ["Length16k.StringLengthConstraint"]);
    // Nonzero accepted-call counts are the positive control for the call-site probes.
    assert!(bootstrap["node_creates"].as_u64().unwrap() > 0);
    assert!(bootstrap["smartlink_creates"].as_u64().unwrap() > 0);
    let mut state = TestExecutionState::new(
        runtime,
        tx_id.clone(),
        case.test_session_state.get_transient_holons().clone(),
        case.test_session_state.fixture_head_index().clone(),
    );
    state.activate_transaction(tx_id).unwrap();
    let mut schema_loaded = false;
    let mut query_loaded = false;
    for step in case.steps.clone() {
        let schema_was_loaded = schema_loaded;
        let commit = matches!(&step, DanceTestStep::Commit { .. });
        let rejection = matches!(
            &step,
            DanceTestStep::Commit { expected_status: ExpectedCommitStatus::Rejected, .. }
        );
        let before: (u32, u32) = if rejection {
            backend
                .conductor
                .call(&backend.cell.zome("holons_test_probes"), "commit_write_counts_for_test", ())
                .await
        } else {
            (0, 0)
        };
        let mut segment = case.clone();
        segment.steps = vec![step];
        run_dance_test_case(segment, &mut state, &mut schema_loaded, &mut query_loaded).await;
        // Domain loading and explicit Commit each produce one request-local audit.
        if commit || (!schema_was_loaded && schema_loaded) {
            let value = audit(&mut signals).await;
            if rejection {
                assert_eq!(value["violations"], EXPECTED_SCALAR_VIOLATIONS);
                for count in ["node_creates", "node_updates", "smartlink_creates"] {
                    assert_eq!(
                        value[count], 0,
                        "no node create/update or SmartLink create attempted: {value}"
                    );
                }
            } else {
                assert_eq!(value["violations"], 0);
                assert!(value["node_creates"].as_u64().unwrap() > 0);
            }
        }
        if rejection {
            let after: (u32, u32) = backend
                .conductor
                .call(&backend.cell.zome("holons_test_probes"), "commit_write_counts_for_test", ())
                .await;
            assert_eq!(after, before, "no entry create/update or link create authored");
        }
    }
    assert!(
        signals.try_recv().is_err(),
        "unexpected extra CommitAudit signal; audit alignment changed"
    );
    collector.abort();
}
