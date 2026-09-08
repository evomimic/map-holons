//! MAP Dance Test Cases
//!
//! The functions in this file are used in conjunction with Rust rstest test fixtures.
//! Tracing is initialized automatically by the test harness.
//! export RUST_LOG to the desired client-side tracing level to include in output.
//! export WASM_LOG to the desired guest-side tracing level to include in output.
//! In increasing level of detail:
//! error, warn, info, debug, trace

//! Examples:

//! To show DEBUG level trace messages on the client-side and WARN level trace messages on the guest-side:
//! export RUST_LOG=debug
//! export WASM_LOG=warn

//! To show INFO level trace messages on the client-side and DEBUG level trace messages on the guest-side:
//! export RUST_LOG=info
//! export WASM_LOG=debug

mod execution_steps;
mod fixture_cases;

use rstest::*;
use tracing::{
    // error,
    info,
    // trace,
    // warn,
    // Level
};

use execution_steps::abandon_staged_changes_executor::execute_abandon_staged_changes;
use execution_steps::add_related_holons_executor::execute_add_related_holons;
use execution_steps::begin_transaction_executor::execute_begin_transaction;
use execution_steps::command_affordance_verification_executor::execute_verify_core_schema_command_affordances;
use execution_steps::commit_executor::execute_commit;
use execution_steps::delete_holon_executor::execute_delete_holon;
use execution_steps::descriptor_verification_executor::{
    execute_verify_book_person_descriptors, execute_verify_book_person_instance_links,
    execute_verify_book_person_smartlink_commit_cache_links,
    execute_verify_core_schema_descriptor_subtypes, execute_verify_core_schema_descriptors,
    execute_verify_core_schema_value_semantics, execute_verify_relationship_anchoring,
    execute_verify_validation_bindings_descriptor_contract,
};
use execution_steps::ensure_database_count_executor::execute_ensure_database_count;
use execution_steps::load_book_person_inverse_test_schema_executor::execute_load_book_person_inverse_test_schema;
use execution_steps::load_book_person_inverse_test_schema_executor::execute_load_inverse_oriented_book_person_instances_expect_failure;
use execution_steps::load_core_schema_executor::{
    execute_load_core_schema, execute_load_generated_commands_schema,
    execute_load_generated_core_schema, execute_load_generated_dance_schema,
    execute_load_generated_query_dance_schema, execute_load_generated_query_schema,
    execute_load_generated_validation_schema,
};
use execution_steps::load_holons_internal_executor::execute_load_holons_internal;
use execution_steps::lookup_saved_holon_executor::execute_lookup_saved_holon_by_key;
use execution_steps::match_db_content_executor::execute_match_db_content;
use execution_steps::new_holon_executor::execute_new_holon;
use execution_steps::query_relationships_executor::execute_query_relationships;
use execution_steps::remove_properties_executor::execute_remove_properties;
use execution_steps::remove_related_holon_executor::execute_remove_related_holons;
use execution_steps::stage_new_from_clone_executor::execute_stage_new_from_clone;
use execution_steps::stage_new_holon_executor::execute_stage_new_holon;
use execution_steps::stage_new_version_executor::execute_stage_new_version;
use execution_steps::with_properties_executor::execute_with_properties;

use fixture_cases::abandon_staged_changes_fixture::*;
use fixture_cases::bootstrap_operational_schema_fixture::*;
use fixture_cases::delete_holon_fixture::*;
use fixture_cases::ergonomic_add_remove_properties_fixture::*;
use fixture_cases::ergonomic_add_remove_related_holons_fixture::*;
use fixture_cases::load_book_person_inverse_schema_fixture::*;
use fixture_cases::load_holons_internal_fixture::*;
use fixture_cases::simple_add_remove_properties_fixture::*;
use fixture_cases::simple_add_remove_related_holons_fixture::*;
use fixture_cases::simple_create_holon_fixture::*;
use fixture_cases::smartlink_commit_cache_fixture::*;
use fixture_cases::stage_new_from_clone_fixture::*;
use fixture_cases::stage_new_version_fixture::*;
use fixture_cases::transaction_lifecycle_fixture::*;

use self::execution_steps::execute_print_database;
use holons_test::execution_state::TestExecutionState;
use holons_test::harness::helpers::TEST_CLIENT_PREFIX;
use holons_test::harness::prelude::{DanceTestStep, DancesTestCase};

use holons_test::harness::helpers::init_test_runtime;

use map_commands_contract::{MapCommand, MapResult, SpaceCommand};
use map_commands_runtime::ExecutionPolicy;

/// Dance Sweettests share a bootstrapped runtime within each suite. Every
/// scenario still receives its own transaction and fixture execution registry.
///
/// To selectively run JUST THE TESTS in this file, use:
///      cargo test -p dances --test dance_tests
///      set RUST_LOG to enable client-side (i.e., test code) tracing
///      set WASM_LOG to enable guest-side (i.e., zome code) tracing
///
#[rstest]
#[case::pristine_bootstrap_and_loader(pristine_bootstrap_and_loader_suite())]
#[case::runtime_behavior_matrix(runtime_behavior_matrix_suite())]
#[tokio::test(flavor = "multi_thread")]
async fn rstest_dance_test_suites(#[case] suite: DanceTestSuite) {
    run_dance_test_suite(suite).await;
}

struct DanceTestSuite {
    name: &'static str,
    test_cases: Vec<DancesTestCase>,
}

fn pristine_bootstrap_and_loader_suite() -> DanceTestSuite {
    DanceTestSuite {
        name: "pristine_bootstrap_and_loader",
        test_cases: vec![
            bootstrap_operational_schema_fixture().unwrap(),
            loader_incremental_fixture().unwrap(),
        ],
    }
}

fn runtime_behavior_matrix_suite() -> DanceTestSuite {
    DanceTestSuite {
        name: "runtime_behavior_matrix",
        test_cases: vec![
            load_book_person_inverse_schema_fixture().unwrap(),
            stage_new_version_fixture().unwrap(),
            simple_create_holon_fixture().unwrap(),
            simple_abandon_staged_changes_fixture().unwrap(),
            simple_add_remove_properties_fixture().unwrap(),
            simple_add_remove_related_holons_fixture().unwrap(),
            ergonomic_add_remove_properties_fixture().unwrap(),
            ergonomic_add_remove_related_holons_fixture().unwrap(),
            stage_new_from_clone_fixture().unwrap(),
            transaction_lifecycle_fixture().unwrap(),
            smartlink_commit_cache_fixture().unwrap(),
            delete_holon_fixture().unwrap(),
            frozen_member_head_redirect_fixture().unwrap(),
            frozen_member_head_redirect_cross_tx_fixture().unwrap(),
            cross_transaction_staged_target_diagnostic_fixture().unwrap(),
        ],
    }
}

/// Boots one fresh runtime, then executes each finalized scenario through its own
/// transaction and execution registry. Scenario fixtures remain declarative and
/// self-contained; only the immutable Core Schema bootstrap is amortized.
async fn run_dance_test_suite(test_suite: DanceTestSuite) {
    assert!(!test_suite.test_cases.is_empty(), "a Dance test suite needs at least one scenario");
    info!("Starting Dance test suite: {}", test_suite.name);

    let mut bootstrap_case = DancesTestCase::default();
    let (runtime, initial_tx_id) = init_test_runtime(&mut bootstrap_case).await;
    let mut book_person_schema_loaded = false;

    for (scenario_index, test_case) in test_suite.test_cases.into_iter().enumerate() {
        let tx_id = if scenario_index == 0 {
            initial_tx_id.clone()
        } else {
            let result = runtime
                .execute_command(
                    MapCommand::Space(SpaceCommand::BeginTransaction),
                    ExecutionPolicy::default(),
                )
                .await
                .expect("failed to begin a scenario transaction");
            match result {
                MapResult::TransactionCreated { tx_id } => tx_id,
                other => panic!("expected TransactionCreated, got {other:?}"),
            }
        };

        let fixture_transient_holons = test_case.test_session_state.get_transient_holons().clone();
        let fixture_head_index = test_case.test_session_state.fixture_head_index().clone();
        let mut test_execution_state = TestExecutionState::new(
            runtime.clone(),
            tx_id.clone(),
            fixture_transient_holons,
            fixture_head_index,
        );
        test_execution_state
            .activate_transaction(tx_id)
            .expect("failed to import scenario fixture holons");
        run_dance_test_case(test_case, &mut test_execution_state, &mut book_person_schema_loaded)
            .await;
    }
}

/// Drives a finalized `DancesTestCase` through step execution in an initialized runtime.
async fn run_dance_test_case(
    test_case: DancesTestCase,
    mut test_execution_state: &mut TestExecutionState,
    book_person_schema_loaded: &mut bool,
) {
    // The heavy lifting for this test is in the test data set creation.

    assert!(
        test_case.is_finalized(),
        "DancesTestCase must be finalized before execution. Call test_case.finalize(&fixture_context, &fixture_holons) in the fixture."
    );
    info!("\n\n{TEST_CLIENT_PREFIX} ******* STARTING {} TEST CASE WITH {} TEST STEPS ***************************", test_case.name, test_case.steps.len());
    info!("\n   Test Case Description: {}", test_case.description);

    info!("Planned Steps:");
    for (i, step) in test_case.steps.iter().enumerate() {
        info!(" {}. {}", i + 1, step);
    }

    for step in test_case.steps {
        info!("========== STARTING STEP: {}", step);

        match step {
            DanceTestStep::AbandonStagedChanges { step_token, expected_error, .. } => {
                execute_abandon_staged_changes(
                    &mut test_execution_state,
                    step_token,
                    expected_error,
                )
                .await
            }
            DanceTestStep::AddRelatedHolons {
                step_token,
                relationship_name,
                holons_to_add,
                expected_error,
                ..
            } => {
                execute_add_related_holons(
                    &mut test_execution_state,
                    step_token,
                    relationship_name,
                    holons_to_add,
                    expected_error,
                )
                .await
            }
            DanceTestStep::BeginTransaction { expected_error, .. } => {
                execute_begin_transaction(&mut test_execution_state, expected_error).await
            }
            DanceTestStep::Commit { saved_tokens, expected_status, expected_error, .. } => {
                execute_commit(
                    &mut test_execution_state,
                    saved_tokens,
                    expected_status,
                    expected_error,
                )
                .await
            }
            DanceTestStep::DeleteHolon { step_token, expected_error, .. } => {
                execute_delete_holon(&mut test_execution_state, step_token, expected_error).await
            }
            DanceTestStep::EnsureDatabaseCount { expected_count, .. } => {
                execute_ensure_database_count(&mut test_execution_state, expected_count).await
            }
            DanceTestStep::LoadHolonsInternal {
                set_id,
                expect_staged,
                expect_committed,
                expect_links_created,
                expect_errors,
                expect_total_bundles,
                expect_total_loader_holons,
                expect_status,
            } => {
                execute_load_holons_internal(
                    &mut test_execution_state,
                    set_id,
                    expect_staged,
                    expect_committed,
                    expect_links_created,
                    expect_errors,
                    expect_total_bundles,
                    expect_total_loader_holons,
                    expect_status,
                )
                .await
            }
            DanceTestStep::LookupSavedHolonByKey { step_token, key, expected_error, .. } => {
                execute_lookup_saved_holon_by_key(
                    &mut test_execution_state,
                    step_token,
                    key,
                    expected_error,
                )
                .await
            }
            DanceTestStep::LoadCoreSchema { .. } => {
                execute_load_core_schema(&mut test_execution_state).await
            }
            DanceTestStep::LoadGeneratedCoreSchema { .. } => {
                execute_load_generated_core_schema(&mut test_execution_state).await
            }
            DanceTestStep::LoadGeneratedDanceSchema { .. } => {
                execute_load_generated_dance_schema(&mut test_execution_state).await
            }
            DanceTestStep::LoadGeneratedCommandsSchema { .. } => {
                execute_load_generated_commands_schema(&mut test_execution_state).await
            }
            DanceTestStep::LoadGeneratedValidationSchema { .. } => {
                execute_load_generated_validation_schema(&mut test_execution_state).await
            }
            DanceTestStep::LoadGeneratedQuerySchema { .. } => {
                execute_load_generated_query_schema(&mut test_execution_state).await
            }
            DanceTestStep::LoadGeneratedQueryDanceSchema { .. } => {
                execute_load_generated_query_dance_schema(&mut test_execution_state).await
            }
            DanceTestStep::LoadBookPersonInverseTestSchema { .. } => {
                if *book_person_schema_loaded {
                    info!("Book/Person inverse test schema is already available in this suite");
                } else {
                    execute_load_book_person_inverse_test_schema(&mut test_execution_state).await;
                    *book_person_schema_loaded = true;
                }
            }
            DanceTestStep::LoadInverseOrientedBookPersonInstancesExpectFailure { .. } => {
                execute_load_inverse_oriented_book_person_instances_expect_failure(
                    &mut test_execution_state,
                )
                .await
            }
            DanceTestStep::VerifyBookPersonDescriptors { .. } => {
                execute_verify_book_person_descriptors(&mut test_execution_state).await
            }
            DanceTestStep::VerifyBookPersonInstanceLinks { .. } => {
                execute_verify_book_person_instance_links(&mut test_execution_state).await
            }
            DanceTestStep::VerifyBookPersonSmartLinkCommitCacheLinks { .. } => {
                execute_verify_book_person_smartlink_commit_cache_links(&mut test_execution_state)
                    .await
            }
            DanceTestStep::VerifyRelationshipAnchoring { .. } => {
                execute_verify_relationship_anchoring(&mut test_execution_state).await
            }
            DanceTestStep::VerifyCoreSchemaDescriptorSubtypes { .. } => {
                execute_verify_core_schema_descriptor_subtypes(&mut test_execution_state).await
            }
            DanceTestStep::VerifyCoreSchemaDescriptors { .. } => {
                execute_verify_core_schema_descriptors(&mut test_execution_state).await
            }
            DanceTestStep::VerifyCoreSchemaCommandAffordances { .. } => {
                execute_verify_core_schema_command_affordances(&mut test_execution_state).await
            }
            DanceTestStep::VerifyCoreSchemaValueSemantics { .. } => {
                execute_verify_core_schema_value_semantics(&mut test_execution_state).await
            }
            DanceTestStep::VerifyValidationBindingsDescriptorContract { .. } => {
                execute_verify_validation_bindings_descriptor_contract(&mut test_execution_state)
                    .await
            }
            DanceTestStep::MatchSavedContent => {
                execute_match_db_content(&mut test_execution_state).await
            }
            DanceTestStep::NewHolon { step_token, properties, key, expected_error, .. } => {
                execute_new_holon(
                    &mut test_execution_state,
                    step_token,
                    properties,
                    key,
                    expected_error,
                )
                .await
            }
            DanceTestStep::PrintDatabase => execute_print_database(&mut test_execution_state).await,
            DanceTestStep::QueryRelationships {
                step_token,
                query_expression,
                expected_error,
                ..
            } => {
                execute_query_relationships(
                    &mut test_execution_state,
                    step_token,
                    query_expression,
                    expected_error,
                )
                .await
            }
            DanceTestStep::RemoveProperties { step_token, properties, expected_error, .. } => {
                execute_remove_properties(
                    &mut test_execution_state,
                    step_token,
                    properties,
                    expected_error,
                )
                .await
            }
            DanceTestStep::RemoveRelatedHolons {
                step_token,
                relationship_name,
                holons_to_remove,
                expected_error,
                ..
            } => {
                execute_remove_related_holons(
                    &mut test_execution_state,
                    step_token,
                    relationship_name,
                    holons_to_remove,
                    expected_error,
                )
                .await
            }
            DanceTestStep::StageHolon { step_token, expected_error, .. } => {
                execute_stage_new_holon(&mut test_execution_state, step_token, expected_error).await
            }
            DanceTestStep::StageNewFromClone { step_token, new_key, expected_error, .. } => {
                execute_stage_new_from_clone(
                    &mut test_execution_state,
                    step_token,
                    new_key,
                    expected_error,
                )
                .await
            }
            DanceTestStep::StageNewVersion {
                step_token,
                expected_error,
                version_count,
                expected_staging_error,
                ..
            } => {
                execute_stage_new_version(
                    &mut test_execution_state,
                    step_token,
                    expected_error,
                    version_count,
                    expected_staging_error,
                )
                .await
            }
            DanceTestStep::WithProperties { step_token, properties, expected_error, .. } => {
                execute_with_properties(
                    &mut test_execution_state,
                    step_token,
                    properties,
                    expected_error,
                )
                .await
            }
        }
    }
    info!("\n{TEST_CLIENT_PREFIX} ------- END OF {} TEST CASE  ---------------", test_case.name);
}
