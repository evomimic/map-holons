use holons_prelude::prelude::*;
use holons_test::TestExecutionState;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use pretty_assertions::assert_eq;
use tracing::{debug, info};

/// Verifies the database holon count via `TransactionAction::GetAllHolons`.
pub async fn execute_ensure_database_count(
    state: &mut TestExecutionState,
    expected_count: MapInteger,
) {
    let context = state
        .open_assertion_context("ensure_database_count")
        .await
        .expect("failed to open assertion transaction");

    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::GetAllHolons,
    });
    let result =
        state.dispatch_command(command, "get_all_holons").await.expect("get_all_holons failed");

    let collection = match result {
        MapResult::Collection(c) => c,
        other => panic!("Expected Collection, got {:?}", other),
    };

    let actual_count = collection.get_count();
    debug!(
        "--- ensure_db_count: Expected: {:?}, Retrieved: {:?} ---",
        expected_count, actual_count.0
    );

    assert_eq!(expected_count, actual_count);
    info!("Success! DB count matched expected");
}
