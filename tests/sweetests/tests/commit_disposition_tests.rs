//! Expected harness failures must name dispositions before downstream assertions.

mod execution_steps;
mod fixture_cases {
    pub mod commit_disposition_fixture;
    pub mod described_instances;
}

use execution_steps::dance_test_runner::{run_dance_test_suite, DanceTestSuite};
use fixture_cases::commit_disposition_fixture::{
    commit_stale_graph_only_declaration_fixture, commit_unsupported_pass_one_fixture,
};

// Both panic strings include disposition_report's Debug punctuation for Option<MapString> keys.
#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = "Book.StaleGraphOnly\"))): declared GraphOnly, observed NewVersion")]
async fn property_mutation_reports_stale_graph_only_disposition() {
    run_dance_test_suite(DanceTestSuite {
        name: "stale_graph_only_declaration",
        test_cases: vec![commit_stale_graph_only_declaration_fixture().unwrap()],
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(
    expected = "Book.UnsupportedPassOne\"))): declared NewRoot, observed Unsupported: Pass 1 did not reach a Saved outcome"
)]
async fn pass_one_failure_reports_unsupported_candidate() {
    run_dance_test_suite(DanceTestSuite {
        name: "unsupported_pass_one_failure",
        test_cases: vec![commit_unsupported_pass_one_fixture().unwrap()],
    })
    .await;
}
