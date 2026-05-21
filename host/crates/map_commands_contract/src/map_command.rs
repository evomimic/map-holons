use super::{CommandLifecyclePolicy, HolonCommand, SpaceCommand, TransactionCommand};

/// Post-binding domain command.
///
/// Contains resolved runtime objects (transaction handles, holon references).
/// No `*Wire` types appear below the binding seam.
#[derive(Debug)]
pub enum MapCommand {
    Space(SpaceCommand),
    Transaction(TransactionCommand),
    Holon(HolonCommand),
}

impl MapCommand {
    pub fn policy(&self) -> CommandLifecyclePolicy {
        match self {
            MapCommand::Space(cmd) => cmd.policy(),
            MapCommand::Transaction(cmd) => cmd.action.policy(),
            MapCommand::Holon(cmd) => cmd.action.policy(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            MapCommand::Space(cmd) => cmd.label(),
            MapCommand::Transaction(cmd) => cmd.action.label(),
            MapCommand::Holon(cmd) => cmd.action.label(),
        }
    }
}
