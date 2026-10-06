//! Shared execution of declarative dance suites, including expected harness failures.

use super::abandon_staged_changes_executor::execute_abandon_staged_changes;
use super::add_related_holons_executor::execute_add_related_holons;
use super::begin_transaction_executor::execute_begin_transaction;
use super::command_affordance_verification_executor::execute_verify_core_schema_command_affordances;
use super::commit_executor::{
    execute_commit, execute_verify_commit_carrier_finding, execute_verify_commit_rejection,
};
use super::delete_holon_executor::execute_delete_holon;
use super::descriptor_verification_executor::{
    execute_verify_book_person_descriptors, execute_verify_book_person_instance_links,
    execute_verify_book_person_smartlink_commit_cache_links,
    execute_verify_core_schema_descriptor_subtypes, execute_verify_core_schema_descriptors,
    execute_verify_core_schema_value_semantics, execute_verify_relationship_anchoring,
    execute_verify_validation_bindings_descriptor_contract,
};
use super::ensure_database_count_executor::execute_ensure_database_count;
use super::load_book_person_inverse_test_schema_executor::execute_load_book_person_inverse_test_schema;
use super::load_book_person_inverse_test_schema_executor::execute_load_inverse_oriented_book_person_instances_expect_failure;
use super::load_core_schema_executor::{
    execute_load_core_schema, execute_load_generated_commands_schema,
    execute_load_generated_core_schema, execute_load_generated_dance_schema,
    execute_load_generated_query_dance_schema, execute_load_generated_query_schema,
    execute_load_generated_validation_schema,
};
use super::load_holons_internal_executor::execute_load_holons_internal;
use super::load_query_test_schema_executor::execute_load_query_test_schema;
use super::lookup_saved_holon_executor::execute_lookup_saved_holon_by_key;
use super::match_db_content_executor::execute_match_db_content;
use super::new_holon_executor::execute_new_holon;
use super::persisted_graph_executor::execute_verify_persisted_graph;
use super::query_executor::execute_query;
use super::query_relationships_executor::execute_query_relationships;
use super::remove_properties_executor::execute_remove_properties;
use super::remove_related_holon_executor::execute_remove_related_holons;
use super::schema_validation_executor::execute_verify_schema_validation_conformance;
use super::stage_new_from_clone_executor::execute_stage_new_from_clone;
use super::stage_new_holon_executor::execute_stage_new_holon;
use super::stage_new_version_executor::execute_stage_new_version;
use super::with_descriptor_executor::execute_with_descriptor;
use super::with_properties_executor::execute_with_properties;

use super::execute_print_database;
use holons_test::execution_state::TestExecutionState;
use holons_test::harness::helpers::TEST_CLIENT_PREFIX;
use holons_test::harness::prelude::{DanceTestStep, DancesTestCase};

use holons_test::harness::helpers::init_test_runtime;

use map_commands_contract::{MapCommand, MapResult, SpaceCommand};
use map_commands_runtime::ExecutionPolicy;

use tracing::info;

/// Self-contained scenarios sharing one bootstrapped runtime.
pub struct DanceTestSuite {
    pub name: &'static str,
    pub test_cases: Vec<DancesTestCase>,
}

/// Boots one fresh runtime, then executes each finalized scenario through its own
/// transaction and execution registry. Scenario fixtures remain declarative and
/// self-contained; only the immutable Core Schema bootstrap is amortized.
pub async fn run_dance_test_suite(test_suite: DanceTestSuite) {
    assert!(!test_suite.test_cases.is_empty(), "a Dance test suite needs at least one scenario");
    info!("Starting Dance test suite: {}", test_suite.name);

    let mut bootstrap_case = DancesTestCase::default();
    let (runtime, initial_tx_id) = init_test_runtime(&mut bootstrap_case).await;
    let mut book_person_schema_loaded = false;
    let mut query_test_schema_loaded = false;

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
        run_dance_test_case(
            test_case,
            &mut test_execution_state,
            &mut book_person_schema_loaded,
            &mut query_test_schema_loaded,
        )
        .await;
    }
}

/// Drives a finalized `DancesTestCase` through step execution in an initialized runtime.
pub async fn run_dance_test_case(
    test_case: DancesTestCase,
    mut test_execution_state: &mut TestExecutionState,
    book_person_schema_loaded: &mut bool,
    query_test_schema_loaded: &mut bool,
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
            DanceTestStep::WithDescriptor { step_token, descriptor, expected_error, .. } => {
                execute_with_descriptor(
                    &mut test_execution_state,
                    step_token,
                    descriptor,
                    expected_error,
                )
                .await
            }
            DanceTestStep::BeginTransaction { expected_error, .. } => {
                execute_begin_transaction(&mut test_execution_state, expected_error).await
            }
            DanceTestStep::Commit {
                candidates,
                retry_participants,
                expected_status,
                expected_error,
                ..
            } => {
                execute_commit(
                    &mut test_execution_state,
                    candidates,
                    retry_participants,
                    expected_status,
                    expected_error,
                )
                .await
            }
            DanceTestStep::DeleteHolon { step_token, expected_error, .. } => {
                execute_delete_holon(&mut test_execution_state, step_token, expected_error).await
            }
            DanceTestStep::VerifyCommitRejection {
                rejected_holons,
                expected_violation_count,
                ..
            } => execute_verify_commit_rejection(
                &test_execution_state,
                rejected_holons,
                expected_violation_count,
            ),
            DanceTestStep::VerifyCommitCarrierFinding { expected, .. } => {
                execute_verify_commit_carrier_finding(&test_execution_state, expected)
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
                expect_validation_violation_count,
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
                    expect_validation_violation_count,
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
            DanceTestStep::LoadQueryTestSchema { .. } => {
                if *query_test_schema_loaded {
                    info!("Query test schema is already available in this suite");
                } else {
                    execute_load_query_test_schema(&mut test_execution_state).await;
                    *query_test_schema_loaded = true;
                }
            }
            DanceTestStep::ExecuteQuery { query, input, route, bindings, expectation, .. } => {
                execute_query(&mut test_execution_state, query, input, route, bindings, expectation)
                    .await
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
            DanceTestStep::VerifyPersistedGraph { expected, .. } => {
                execute_verify_persisted_graph(&mut test_execution_state, expected).await
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
            DanceTestStep::VerifySchemaValidationConformance { .. } => {
                execute_verify_schema_validation_conformance(&mut test_execution_state).await
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
