//! Public loader coverage for the interim required-Core-anchor publication bound.
// Shared step executors include suite entrypoints unused by this isolated case.
#[allow(dead_code)]
mod execution_steps;
#[allow(dead_code)]
mod fixture_cases {
    pub mod commit_schema_fixture;
    pub mod described_instance_helpers;
    pub mod load_holons_internal_fixture;
    pub mod schema_authoring_helpers;
}

use core_types::CommitValidationViolationKind;
use execution_steps::dance_test_runner::run_dance_test_case;
use fixture_cases::commit_schema_fixture::loader_required_anchor_update_fixture;
use holons_core::core_shared_objects::holon::StagedState;
use holons_core::RelationshipReadHint;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::init_test_runtime;
use holons_test::{DancesTestCase, TestExecutionState};
use map_commands_contract::{MapCommand, MapResult, SpaceCommand};

#[tokio::test(flavor = "multi_thread")]
async fn loader_anchor_edit_is_a_rejected_successor_without_persisted_changes() {
    let case = loader_required_anchor_update_fixture().unwrap();
    let mut bootstrap = DancesTestCase::default();
    let (runtime, tx_id) = init_test_runtime(&mut bootstrap).await;
    let mut state = TestExecutionState::new(
        runtime,
        tx_id.clone(),
        case.test_session_state.get_transient_holons().clone(),
        case.test_session_state.fixture_head_index().clone(),
    );
    state.activate_transaction(tx_id).unwrap();
    run_dance_test_case(case, &mut state, &mut false, &mut false).await;
    let context = state.context();
    let candidates = context.staged_references().unwrap();
    let update = candidates
        .iter()
        .find(|candidate| candidate.versioned_source_id().unwrap().is_some())
        .expect("promoted saved write source");
    assert_eq!(update.staged_state().unwrap(), StagedState::ForUpdateNewVersion);
    let source = HolonReference::smart_from_id(
        context.space_read_handle(),
        HolonId::Local(update.versioned_source_id().unwrap().unwrap()),
    );
    assert_eq!(source.key().unwrap().unwrap().0, "HolonType.TypeDescriptor");
    assert!(update.validation_findings().unwrap().iter().any(
        |finding| matches!(&finding.kind, CommitValidationViolationKind::RuleViolation { code }
            if code == "UnsupportedRequiredAnchorUpdate")
            && finding.rule_key.is_none()
    ));
    let result = state
        .dispatch_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            "fresh anchor observer",
        )
        .await
        .unwrap();
    let MapResult::TransactionCreated { tx_id } = result else { panic!("observer transaction") };
    let observer = state.runtime().session().get_transaction(&tx_id).unwrap();
    let source =
        HolonReference::smart_from_id(observer.space_read_handle(), source.holon_id().unwrap());
    assert!(source
        .related_holons_with_hint(
            CoreRelationshipTypeName::Successor,
            RelationshipReadHint::RequireFresh,
        )
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .is_empty());
    let saved_members = source
        .related_holons_with_hint(
            CoreRelationshipTypeName::InstanceProperties,
            RelationshipReadHint::RequireFresh,
        )
        .unwrap();
    assert!(!saved_members
        .read()
        .unwrap()
        .get_members()
        .iter()
        .any(|member| member.key().unwrap().is_some_and(|key| key.0 == "Title.PropertyType")));
    assert!(candidates
        .iter()
        .all(|candidate| !matches!(candidate.staged_state().unwrap(), StagedState::Committed(_))));
}
