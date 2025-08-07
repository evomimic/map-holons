use integrity_core_types::{short_hash, LocalId};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OutboundProxyId(pub LocalId);

impl From<LocalId> for OutboundProxyId {
    fn from(action_hash: LocalId) -> Self {
        OutboundProxyId(action_hash)
    }
}

impl fmt::Display for OutboundProxyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match short_hash(&self.0, 6) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "<invalid utf-8>"),
        }
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

#[derive(Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
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
