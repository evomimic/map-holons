use holons_prelude::prelude::*;
use holons_test::{ResolveBy, TestExecutionState, TestReference};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use pretty_assertions::assert_eq;
use tracing::debug;

/// Queries relationships via `TransactionAction::Dance` (temporary fallback).
///
/// `TransactionAction::Query` currently returns `NotImplemented`, so we wrap the
/// existing dance request in `TransactionAction::Dance(...)` to route through Runtime.
///
/// TODO: Migrate to native `TransactionAction::Query` once implemented.
pub async fn execute_query_relationships(
    state: &mut TestExecutionState,
    step_token: TestReference,
    query_expression: QueryExpression,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state
        .open_assertion_context("query_relationships")
        .await
        .expect("failed to open assertion transaction for query_relationships");

    // 1. LOOKUP — resolve source token
    let source_reference: HolonReference =
        state.resolve_execution_reference(&context, ResolveBy::Source, &step_token).unwrap();

    let node_collection =
        NodeCollection { members: vec![Node::new(source_reference, None)], query_spec: None };

    // 2. BUILD — wrap dance request in TransactionAction::Dance
    let dance_request = build_query_relationships_dance_request(node_collection, query_expression)
        .expect("Failed to build query_relationships request");
    debug!("Dance Request (via TransactionAction::Dance): {:#?}", dance_request);

    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::Dance(dance_request),
    });

    // 3. DISPATCH
    let result = state.dispatch_command(command, "query_relationships").await;
    debug!("query_relationships result: {:?}", &result);

    // 4. VALIDATE
    match result {
        Ok(MapResult::DanceResponse(response)) => {
            if expected_error.is_none() {
                assert_eq!(
                    response.status_code,
                    ResponseStatusCode::OK,
                    "query_relationships: unexpected status: {}",
                    response.description,
                );
            } else {
                assert_ne!(
                    response.status_code,
                    ResponseStatusCode::OK,
                    "query_relationships expected failure but got OK",
                );
            }
            // TODO: Match on response.body node collection expected vs actual
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(
                Some(actual),
                expected_error,
                "query_relationships: unexpected error {:?}",
                e,
            );
        }
        Ok(other) => panic!("query_relationships: expected DanceResponse, got {:?}", other),
    }
}
