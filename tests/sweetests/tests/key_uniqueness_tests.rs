//! Loader creates against populated storage are rejected; imports do not reconcile themselves.
#[allow(dead_code)]
mod execution_steps;
use core_types::CommitValidationViolationKind;
use execution_steps::dance_test_runner::run_dance_test_case;
use holons_core::core_shared_objects::holon::StagedState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{build_book_person_inverse_content_set, init_test_runtime};
use holons_test::{DancesTestCase, TestCaseInit, TestExecutionState};
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};

#[tokio::test(flavor = "multi_thread")]
async fn reimporting_domain_creates_is_a_key_uniqueness_rejection() {
    let TestCaseInit { mut test_case, fixture_context, fixture_holons, .. } = TestCaseInit::new(
        "Loader create re-import",
        "Re-importing the same package must stage versions or no-ops explicitly",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None).unwrap();
    test_case.finalize(&fixture_context, &fixture_holons).unwrap();
    let mut bootstrap = DancesTestCase::default();
    let (runtime, tx_id) = init_test_runtime(&mut bootstrap).await;
    let mut state = TestExecutionState::new(
        runtime,
        tx_id.clone(),
        test_case.test_session_state.get_transient_holons().clone(),
        test_case.test_session_state.fixture_head_index().clone(),
    );
    state.activate_transaction(tx_id).unwrap();
    run_dance_test_case(test_case, &mut state, &mut false, &mut false).await;
    let MapResult::TransactionCreated { tx_id } = state
        .dispatch_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            "begin reimport transaction",
        )
        .await
        .unwrap()
    else {
        panic!("transaction")
    };
    state.activate_transaction(tx_id).unwrap();
    let context = state.context();
    let result = state
        .dispatch_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::LoadHolons {
                    content_set: build_book_person_inverse_content_set().unwrap(),
                },
            }),
            "reimport existing domain schema as creates",
        )
        .await
        .unwrap();
    let MapResult::Reference(HolonReference::Transient(response)) = result else {
        panic!("loader response")
    };
    assert_eq!(
        response.property_value(CorePropertyTypeName::LoadCommitStatus).unwrap(),
        Some("Rejected".to_base_value())
    );
    assert_eq!(
        response.property_value(CorePropertyTypeName::HolonsCommitted).unwrap(),
        Some(MapInteger(0).to_base_value())
    );
    assert_eq!(
        response.property_value(CorePropertyTypeName::ErrorCount).unwrap(),
        Some(MapInteger(0).to_base_value())
    );
    let candidates = context.staged_references().unwrap();
    assert!(!candidates.is_empty());
    assert!(candidates
        .iter()
        .all(|c| !matches!(c.staged_state().unwrap(), StagedState::Committed(_))));
    assert!(candidates.iter().any(|c| c.validation_findings().unwrap().iter().any(|v| matches!(&v.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-KEY-006"))));
}
