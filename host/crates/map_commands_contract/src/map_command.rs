use super::{CommandDescriptor, HolonCommand, SpaceCommand, TransactionCommand};

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
    pub fn descriptor(&self) -> CommandDescriptor {
        match self {
            MapCommand::Space(cmd) => cmd.descriptor(),
            MapCommand::Transaction(cmd) => cmd.action.descriptor(),
            MapCommand::Holon(cmd) => cmd.action.descriptor(),
        }
    }
}
