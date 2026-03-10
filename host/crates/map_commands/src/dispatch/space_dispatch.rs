use core_types::HolonError;

use crate::domain::{MapResult, SpaceCommand};

use super::runtime_session::RuntimeSession;

/// Dispatches space-scoped commands.
pub fn dispatch_space(
    session: &RuntimeSession,
    command: SpaceCommand,
) -> Result<MapResult, HolonError> {
    match command {
        SpaceCommand::BeginTransaction => {
            let tx_id = session.begin_transaction()?;
            Ok(MapResult::TransactionCreated { tx_id })
        }
    }
}
