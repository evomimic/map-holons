//! Independent source-chain evidence for semantic Commit rejection.
#[allow(dead_code)]
mod execution_steps;
#[allow(dead_code)]
mod fixture_cases;
use execution_steps::dance_test_runner::run_dance_test_case;
use holons_test::harness::helpers::{init_runtime_with_initiator, setup_probe_enabled_conductor};
use holons_test::{DanceTestStep, ExpectedCommitStatus, TestExecutionState};
use holons_trust_channel::TrustChannel;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires probe-enabled conductor artifacts"]
async fn semantic_rejections_author_no_records() {
    use fixture_cases::*;
    let mut cases = [
        commit_enum_conformance_fixture::commit_enum_conformance_fixture().unwrap(),
        commit_default_declaration_fixture::commit_default_declaration_fixture().unwrap(),
        commit_conformance_fixture::commit_conformance_fixture().unwrap(),
        commit_enum_evolution_fixture::commit_enum_evolution_fixture().unwrap(),
    ];
    let backend = setup_probe_enabled_conductor().await;
    let initiator = Arc::new(TrustChannel::new(backend.clone()));
    let (runtime, initial_tx_id) = init_runtime_with_initiator(&mut cases[0], initiator).await;
    let mut schema_loaded = false;
    let mut query_loaded = false;
    for (index, case) in cases.into_iter().enumerate() {
        let tx_id = if index == 0 {
            initial_tx_id.clone()
        } else {
            use map_commands_contract::{MapCommand, MapResult, SpaceCommand};
            let result = runtime
                .execute_command(
                    MapCommand::Space(SpaceCommand::BeginTransaction),
                    map_commands_runtime::ExecutionPolicy::default(),
                )
                .await
                .unwrap();
            let MapResult::TransactionCreated { tx_id } = result else {
                panic!("expected scenario transaction")
            };
            tx_id
        };
        let mut state = TestExecutionState::new(
            runtime.clone(),
            tx_id.clone(),
            case.test_session_state.get_transient_holons().clone(),
            case.test_session_state.fixture_head_index().clone(),
        );
        state.activate_transaction(tx_id).unwrap();
        for step in case.steps.clone() {
            let rejected = matches!(
                &step,
                DanceTestStep::Commit { expected_status: ExpectedCommitStatus::Rejected, .. }
            );
            let before: (u32, u32) = if rejected {
                backend
                    .conductor
                    .call(
                        &backend.cell.zome("holons_test_probes"),
                        "commit_write_counts_for_test",
                        (),
                    )
                    .await
            } else {
                (0, 0)
            };
            if rejected {
                assert!(
                    before.0 > 0 && before.1 > 0,
                    "bootstrap provides the write-probe positive control"
                );
            }
            let mut segment = case.clone();
            segment.steps = vec![step];
            run_dance_test_case(segment, &mut state, &mut schema_loaded, &mut query_loaded).await;
            if rejected {
                let after: (u32, u32) = backend
                    .conductor
                    .call(
                        &backend.cell.zome("holons_test_probes"),
                        "commit_write_counts_for_test",
                        (),
                    )
                    .await;
                assert_eq!(
                    after, before,
                    "{}: rejected Commit must author no entry or link",
                    case.name
                );
            }
        }
    }
}
