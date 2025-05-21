use serde::{Deserialize, Serialize};
use std::fmt;

use crate::HolonError;

use super::saved_holon_node::SavedHolonNode;

#[derive(Debug)]
pub enum AccessType {
    Abandon,
    Clone,
    Commit,
    Read,
    Write,
}
impl fmt::Display for AccessType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AccessType::Abandon => write!(f, "Abandon"),
            AccessType::Clone => write!(f, "Clone"),
            AccessType::Commit => write!(f, "Commit"),
            AccessType::Read => write!(f, "Read"),
            AccessType::Write => write!(f, "Write"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum HolonState {
    Mutable,
    Immutable,
}

impl HolonState {
    pub fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        use AccessType::*;
        match (self, &access_type) {
            (HolonState::Mutable, _) => Ok(()),

            (HolonState::Immutable, Read | Clone | Commit | Abandon) => Ok(()),

            (HolonState::Immutable, Write ) => Err(HolonError::NotAccessible(
                access_type.to_string(), self.to_string(),
            )),
        }
    }
}

impl fmt::Display for HolonState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HolonState::Mutable => write!(f, "Mutable"),
            HolonState::Immutable => write!(f, "Immutable"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum SavedState {
    Deleted,    // Marked as deleted
    Fetched,    // Retrieved from persistent storage
}

impl fmt::Display for SavedState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SavedState::Deleted => write!(f, "Deleted"),
            SavedState::Fetched => write!(f, "Fetched"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum StagedState {
    /// A Holon that was staged and intentionally abandoned (will not be committed).
    Abandoned,
    /// A Holon that has been successfully committed.
    Committed(SavedHolonNode),
    /// A new Holon that has never been committed before.
    ForCreate,
    /// A Holon cloned from the persistent store for potential modification,
    /// but no changes have been made yet.
    ForUpdate,
    /// A Holon cloned for modification and subsequently changed.
    ForUpdateChanged,
}

impl fmt::Display for StagedState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StagedState::Abandoned => write!(f, "Abandoned"),
            StagedState::Committed(node) => write!(f, "Committed: {:?}", node),
            StagedState::ForCreate => write!(f, "ForCreate"),
            StagedState::ForUpdate => write!(f, "ForUpdate"),
            StagedState::ForUpdateChanged => write!(f, "ForUpdateChanged"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ValidationState {
    NoDescriptor,
    ValidationRequired,
    Validated,
    Invalid,
}
