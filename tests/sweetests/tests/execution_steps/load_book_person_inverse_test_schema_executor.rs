use holons_core::core_shared_objects::holon::StagedState;
use holons_test::harness::helpers::{
    build_book_person_inverse_content_set, build_inverse_oriented_book_person_instance_content_set,
};
use holons_test::TestExecutionState;

use super::load_holons_client_executor::{
    execute_load_holons_client_expect_semantic_rejection, execute_load_holons_client_expect_success,
};

pub async fn execute_load_book_person_inverse_test_schema(test_state: &mut TestExecutionState) {
    let content_set = build_book_person_inverse_content_set().unwrap_or_else(|error| {
        panic!("failed to build Book/Person inverse ContentSet: {error:?}")
    });

    execute_load_holons_client_expect_success(test_state, content_set).await;
}

pub async fn execute_load_inverse_oriented_book_person_instances_expect_failure(
    test_state: &mut TestExecutionState,
) {
    let content_set =
        build_inverse_oriented_book_person_instance_content_set().unwrap_or_else(|error| {
            panic!("failed to build inverse-oriented Book/Person ContentSet: {error:?}")
        });

    execute_load_holons_client_expect_semantic_rejection(test_state, content_set).await;

    // Loader Pass 1 stages nodes and common Commit rejects the independently
    // authored inverse occurrence before persistence. The staged pool remains
    // available for correction in the still-open transaction.
    let context = test_state.context();
    let staged = context.staged_references().expect("returned staged pool");
    assert_eq!(staged.len(), 2);
    for reference in staged {
        assert!(reference.is_in_state(&context, StagedState::ForCreate).unwrap());
        let errors = reference.commit_errors().unwrap();
        assert!(errors.is_empty(), "rejection must not install persistence errors: {errors:?}");
    }
}
