use super::{CommandDescriptor, MutationClassification};

/// Space-scoped domain commands.
///
/// Operate outside any transaction context.
#[derive(Debug)]
pub enum SpaceCommand {
    /// Opens a new transaction.
    BeginTransaction,
}

impl SpaceCommand {
    pub fn descriptor(&self) -> CommandDescriptor {
        match self {
            SpaceCommand::BeginTransaction => CommandDescriptor {
                mutation: MutationClassification::Mutating,
                requires_open_tx: false,
                requires_commit_guard: false,
            },
        }
    }
}
