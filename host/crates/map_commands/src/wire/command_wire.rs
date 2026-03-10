use serde::{Deserialize, Serialize};

use super::{HolonCommandWire, SpaceCommandWire, TransactionCommandWire};

/// Structural command hierarchy for MAP IPC.
///
/// Replaces legacy string-based command routing. Command authority derives
/// from the structural variant, not from strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MapCommandWire {
    Space(SpaceCommandWire),
    Transaction(TransactionCommandWire),
    Holon(HolonCommandWire),
}
