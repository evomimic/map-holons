use hdi::prelude::*; // Dependency on Holochain ActionHash remains for now
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LocalId(pub ActionHash);

impl From<ActionHash> for LocalId {
    fn from(action_hash: ActionHash) -> Self {
        LocalId(action_hash)
    }
}

impl fmt::Display for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", short_hash(&self.0, 6))
    }
}

/// Local utility for display purposes.
/// NOTE: This short_hash helper is duplicated temporarily
/// until Holochain dependencies are abstracted from core_types
/// and unified display helpers can be introduced.

fn short_hash(hash: &ActionHash, length: usize) -> String {
    let full_hash_str = hash.to_string();
    let start_index = full_hash_str.len().saturating_sub(length);
    format!("…{}", &full_hash_str[start_index..])
}
