use core_types::HolonError;

use crate::domain::{MapResult, TransactionAction, TransactionCommand};

use super::runtime_session::RuntimeSession;

/// Dispatches transaction-scoped commands.
pub async fn dispatch_transaction(
    session: &RuntimeSession,
    command: TransactionCommand,
) -> Result<MapResult, HolonError> {
    let tx_id = command.context.tx_id();

    match command.action {
        TransactionAction::Commit => {
            command.context.commit()?;
            session.remove_transaction(&tx_id)?;
            Ok(MapResult::Committed)
        }
        _ => Err(HolonError::NotImplemented(format!(
            "TransactionAction::{:?}",
            command.action
        ))),
    }
}
