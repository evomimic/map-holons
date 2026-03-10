/// Space-scoped domain commands.
///
/// Operate outside any transaction context.
#[derive(Debug)]
pub enum SpaceCommand {
    /// Opens a new transaction.
    BeginTransaction,
}
