use crate::value_types::{BaseValue, MapString};
use derive_new::new;
use hdi::prelude::*;
use std::collections::btree_map::BTreeMap;
use std::fmt;

// ===============================
// 📌 Constants
// ===============================
pub const LOCAL_HOLON_SPACE_PATH: &str = "local_holon_space";
pub const LOCAL_HOLON_SPACE_NAME: &str = "LocalHolonSpace";
pub const LOCAL_HOLON_SPACE_DESCRIPTION: &str = "Default Local Holon Space";

// ===============================
// 📦 Type Aliases
// ===============================
pub type PropertyValue = BaseValue;
pub type PropertyMap = BTreeMap<PropertyName, Option<PropertyValue>>;

// ===============================
// 🌳 HolonNode Struct (holochain EntryType)
// ===============================
#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub original_id: Option<LocalId>,
    pub property_map: PropertyMap,
}

// ===============================
// 🆔 Identifier Types
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LocalId(pub ActionHash);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OutboundProxyId(pub ActionHash);

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ExternalId {
    pub space_id: OutboundProxyId,
    pub local_id: LocalId,
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum HolonId {
    Local(LocalId),
    External(ExternalId),
    // Temporary(TemporaryId)
}

// ===============================
// 🔑 Property Types
// ===============================
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropertyName(pub MapString);

impl fmt::Display for PropertyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ===============================
// ⚡ Helper Functions
// ===============================

/// Returns a shortened version of an `ActionHash` for display purposes.
///
/// This function takes the last `length` characters of the hash
/// and prepends an ellipsis (`…`) to indicate truncation.
///
/// # Arguments
/// * `hash` - The `ActionHash` to shorten.
/// * `length` - The number of characters to display (default is 6).
///
pub fn short_hash(hash: &ActionHash, length: usize) -> String {
    let full_hash_str = hash.to_string();
    let hash_len = full_hash_str.len();
    let start_index = hash_len.saturating_sub(length);
    format!("…{}", &full_hash_str[start_index..])
}

// ===============================
// 🔨 Implementations
// ===============================

// --- LocalId ---
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

// --- OutboundProxyId ---
impl From<ActionHash> for OutboundProxyId {
    fn from(action_hash: ActionHash) -> Self {
        OutboundProxyId(action_hash)
    }
}

impl fmt::Display for OutboundProxyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", short_hash(&self.0, 6))
    }
}

// --- ExternalId ---
impl From<(OutboundProxyId, LocalId)> for ExternalId {
    fn from((space_id, local_id): (OutboundProxyId, LocalId)) -> Self {
        ExternalId { space_id, local_id }
    }
}

impl fmt::Display for ExternalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.space_id, self.local_id)
    }
}

// --- HolonId ---
impl From<LocalId> for HolonId {
    fn from(local_id: LocalId) -> Self {
        HolonId::Local(local_id)
    }
}

impl From<(OutboundProxyId, LocalId)> for HolonId {
    fn from((space_id, local_id): (OutboundProxyId, LocalId)) -> Self {
        HolonId::External(ExternalId { space_id, local_id })
    }
}

impl HolonId {
    pub fn is_local(&self) -> bool {
        matches!(self, HolonId::Local(_))
    }

    pub fn is_external(&self) -> bool {
        matches!(self, HolonId::External(_))
    }

    /// Extracts the `LocalId` from both `Local` and `External` variants.
    pub fn local_id(&self) -> &LocalId {
        match self {
            HolonId::Local(local_id) => local_id,
            HolonId::External(external_id) => &external_id.local_id,
        }
    }

    /// Returns `Some(ExternalId)` for `External` variants, or `None` otherwise.
    pub fn external_id(&self) -> Option<&ExternalId> {
        if let HolonId::External(external_id) = self {
            Some(external_id)
        } else {
            None
        }
    }
}

impl fmt::Display for HolonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HolonId::Local(local_id) => write!(f, "Local({})", local_id),
            HolonId::External(external_id) => write!(f, "External({})", external_id),
        }
    }
}
