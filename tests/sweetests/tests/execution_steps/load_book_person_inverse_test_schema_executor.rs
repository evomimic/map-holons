use holons_core::core_shared_objects::holon::{StagedState, ValidationState};
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    build_book_person_inverse_content_set, build_inverse_oriented_book_person_instance_content_set,
};
use holons_test::TestExecutionState;

use super::load_holons_client_executor::{
    execute_load_holons_client_expect_failure, execute_load_holons_client_expect_success,
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

    execute_load_holons_client_expect_failure(
        test_state,
        content_set,
        // Inverse-oriented input is a loader resolution error, not a Commit
        // validation finding. Accumulating it must short-circuit Commit.
        holons_test::ExpectedLoadStatus::Skipped,
        MapInteger(0),
        &[
            "AuthorOf",
            "declared orientation",
            "opposite endpoint",
            "Person.InverseOrientationFailure.1",
        ],
    )
    .await;

    // Loader Pass 1 may stage nodes, but resolution failure must leave them
    // untouched by Commit. This harness does not measure SmartLink writes;
    // these assertions pin the pre-Commit short-circuit contract instead.
    let context = test_state.context();
    let staged = context.staged_references().expect("returned staged pool");
    assert_eq!(staged.len(), 2);
    for reference in staged {
        assert_eq!(reference.validation_state().unwrap(), ValidationState::ValidationRequired);
        assert!(reference.validation_findings().unwrap().is_empty());
        assert!(reference.is_in_state(&context, StagedState::ForCreate).unwrap());
        let errors = reference.commit_errors().unwrap();
        assert!(errors.is_empty(), "resolver errors belong on the load response: {errors:?}");
    }
}
