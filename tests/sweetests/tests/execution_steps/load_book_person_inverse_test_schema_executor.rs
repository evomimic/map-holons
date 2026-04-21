use holons_prelude::prelude::*;

use holons_test::harness::helpers::{
    build_book_person_inverse_content_set, BOOK_PERSON_INVERSE_METRICS,
};
use holons_test::TestExecutionState;

use super::load_holons_client_executor::execute_load_holons_client;

pub async fn execute_load_book_person_inverse_test_schema(test_state: &mut TestExecutionState) {
    let content_set = build_book_person_inverse_content_set().unwrap_or_else(|error| {
        panic!("failed to build Book/Person inverse ContentSet: {error:?}")
    });

    execute_load_holons_client(
        test_state,
        content_set,
        MapInteger(BOOK_PERSON_INVERSE_METRICS.staged),
        MapInteger(BOOK_PERSON_INVERSE_METRICS.committed),
        MapInteger(BOOK_PERSON_INVERSE_METRICS.links_created),
        MapInteger(BOOK_PERSON_INVERSE_METRICS.errors),
        MapInteger(BOOK_PERSON_INVERSE_METRICS.total_bundles),
        MapInteger(BOOK_PERSON_INVERSE_METRICS.total_loader_holons),
    )
    .await;
}
