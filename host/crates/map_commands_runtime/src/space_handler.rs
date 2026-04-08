use core_types::HolonError;

use map_commands_contract::{MapResult, SpaceCommand};

use super::runtime_session::RuntimeSession;

/// Handles space-scoped commands.
pub fn handle_space(
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
