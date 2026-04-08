use holons_test::execution_state::TestExecutionState;
use integrity_core_types::HolonErrorKind;
use map_commands_contract::{MapCommand, MapResult, SpaceCommand};
use tracing::info;

pub async fn execute_begin_transaction(
    state: &mut TestExecutionState,
    expected_error: Option<HolonErrorKind>,
) {
    let command = MapCommand::Space(SpaceCommand::BeginTransaction);
    let result = state.dispatch_command(command, "begin_transaction").await;

    match result {
        Ok(MapResult::TransactionCreated { tx_id }) => {
            assert!(
                expected_error.is_none(),
                "begin_transaction succeeded but expected {:?}",
                expected_error
            );
            state.activate_transaction(tx_id).expect("failed to activate new transaction");
            info!("New transaction started: {:?}", tx_id);
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(Some(actual), expected_error, "begin_transaction: unexpected error {:?}", e);
        }
        Ok(other) => panic!("begin_transaction: expected TransactionCreated, got {:?}", other),
    }
}
