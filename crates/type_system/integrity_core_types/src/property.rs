use base_types::{BaseValue, MapString};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// ===============================
// 🔑 Property Name
// ===============================

/// A strongly-typed wrapper around MapString for property keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropertyName(pub MapString);

impl fmt::Display for PropertyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ===============================
// 📦 Type Aliases
// ===============================

/// The type of a property’s value at runtime.
pub type PropertyValue = BaseValue;

/// The map from property names to optional property values.
pub type PropertyMap = BTreeMap<PropertyName, PropertyValue>;
