use base_types::MapString;
use std::fmt;
use serde::{Deserialize, Serialize};

// ===============================
// 🔑 Property Types
// ===============================

/// A strongly-typed wrapper around MapString for property keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropertyName(pub MapString);

impl fmt::Display for PropertyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
