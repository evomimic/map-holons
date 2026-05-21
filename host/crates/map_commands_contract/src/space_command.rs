use super::{CommandLifecyclePolicy, MutationClassification};

/// Space-scoped domain commands.
///
/// Operate outside any transaction context.
#[derive(Debug)]
pub enum SpaceCommand {
    /// Opens a new transaction.
    BeginTransaction,
}

impl SpaceCommand {
    pub fn policy(&self) -> CommandLifecyclePolicy {
        match self {
            SpaceCommand::BeginTransaction => CommandLifecyclePolicy {
                mutation: MutationClassification::Mutating,
                requires_open_tx: false,
                requires_commit_guard: false,
            },
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SpaceCommand::BeginTransaction => "begin_transaction",
        }
    }
}
