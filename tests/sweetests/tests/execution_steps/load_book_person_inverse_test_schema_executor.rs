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
        holons_test::ExpectedLoadStatus::Skipped,
        &[
            "AuthorOf",
            "declared orientation",
            "opposite endpoint",
            "Person.InverseOrientationFailure.1",
        ],
    )
    .await;
}
