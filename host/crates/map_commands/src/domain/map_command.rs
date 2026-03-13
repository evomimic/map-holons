use super::{HolonCommand, SpaceCommand, TransactionCommand};

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
