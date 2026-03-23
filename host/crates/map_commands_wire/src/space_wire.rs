use map_commands_contract::SpaceCommand;
use serde::{Deserialize, Serialize};

/// Space-scoped wire commands.
///
/// Space commands operate outside any transaction context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpaceCommandWire {
    /// Opens a new transaction and returns its TxId.
    BeginTransaction,
}

impl SpaceCommandWire {
    /// Binds a space wire command to its domain equivalent.
    ///
    /// Space commands require no context resolution.
    pub fn bind(self) -> SpaceCommand {
        match self {
            SpaceCommandWire::BeginTransaction => SpaceCommand::BeginTransaction,
        }
    }
}

impl From<SpaceCommand> for SpaceCommandWire {
    fn from(cmd: SpaceCommand) -> Self {
        match cmd {
            SpaceCommand::BeginTransaction => SpaceCommandWire::BeginTransaction,
        }
    }
}
