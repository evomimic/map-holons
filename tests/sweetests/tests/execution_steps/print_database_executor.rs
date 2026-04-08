use holons_prelude::prelude::*;
use holons_test::TestExecutionState;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use tracing::info;

/// Retrieves all holons via `TransactionAction::GetAllHolons` and logs them.
pub async fn execute_print_database(state: &mut TestExecutionState) {
    info!("--- TEST STEP: Print Database Contents ---");

    let context = state
        .open_assertion_context("print_database")
        .await
        .expect("failed to open assertion transaction");

    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::GetAllHolons,
    });
    let result =
        state.dispatch_command(command, "get_all_holons").await.expect("get_all_holons failed");

    let holons = match result {
        MapResult::Collection(c) => c,
        other => panic!("Expected Collection, got {:?}", other),
    };

    info!("DB contains {} holons", holons.get_count());

    for holon in holons {
        let key = holon
            .key()
            .map(|key| key.unwrap_or_else(|| MapString("<None>".to_string())))
            .unwrap_or_else(|err| {
                panic!("Attempt to key() resulted in error: {:?}", err);
            });

        info!("Key = {:?}", key.0);
        info!("{:?}", holon.summarize());
    }
}
