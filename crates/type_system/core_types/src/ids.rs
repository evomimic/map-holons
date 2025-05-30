use std::fmt;
use hdi::prelude::*;
use uuid::Uuid;
use integrity_core_types::LocalId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OutboundProxyId(pub ActionHash);

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ExternalId {
    pub space_id: OutboundProxyId,
    pub local_id: LocalId,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TemporaryId(pub Uuid);

impl fmt::Display for TemporaryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HolonId {
    Local(LocalId),
    External(ExternalId),
}

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

    pub fn local_id(&self) -> &LocalId {
        match self {
            HolonId::Local(l) => l,
            HolonId::External(e) => &e.local_id,
        }
    }

    pub fn external_id(&self) -> Option<&ExternalId> {
        if let HolonId::External(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl fmt::Display for HolonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HolonId::Local(l) => write!(f, "Local({})", l),
            HolonId::External(e) => write!(f, "External({})", e),
        }
    }
}

/// Helper for truncating an ActionHash for display.
fn short_hash(hash: &ActionHash, length: usize) -> String {
    let s = hash.to_string();
    let start = s.len().saturating_sub(length);
    format!("…{}", &s[start..])
}