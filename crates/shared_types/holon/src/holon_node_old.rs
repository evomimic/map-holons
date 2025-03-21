use crate::value_types::{BaseValue, MapString};
use derive_new::new;
use hdi::prelude::*;
use std::collections::btree_map::BTreeMap;
use std::fmt;

pub const LOCAL_HOLON_SPACE_PATH: &str = "local_holon_space";
pub const LOCAL_HOLON_SPACE_NAME: &str = "LocalHolonSpace";
pub const LOCAL_HOLON_SPACE_DESCRIPTION: &str = "Default Local Holon Space";

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct HolonNode {
    pub original_id: Option<LocalId>,
    pub property_map: PropertyMap,
}
pub type PropertyValue = BaseValue;
pub type PropertyMap = BTreeMap<PropertyName, PropertyValue>;

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum HolonId {
    Local(LocalId),
    External(ExternalId),
}
/// Construct a (Local variant) of a HolonId from a LocalId
impl From<LocalId> for HolonId {
    fn from(local_id: LocalId) -> Self {
        HolonId::Local(local_id)
    }
}

impl From<(OutboundProxyId, LocalId)> for HolonId {
    fn from(tuple: (OutboundProxyId, LocalId)) -> Self {
        let (space_id, local_id) = tuple;
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

    /// Extracts LocalId from BOTH Local and External HolonIds
    pub fn local_id(&self) -> &LocalId {
        match self {
            HolonId::Local(ref local_id) => local_id,
            HolonId::External(ref external_id) => &external_id.local_id,
        }
    }

    /// Returns Some(ExternalId) from External variants of HolonId and None otherwise
    pub fn external_id(&self) -> Option<&ExternalId> {
        if let HolonId::External(ref external_id) = self {
            Some(external_id)
        } else {
            None
        }
    }
}

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct OutboundProxyId(pub ActionHash);

impl From<ActionHash> for OutboundProxyId {
    fn from(action_hash: ActionHash) -> Self {
        OutboundProxyId(action_hash)
    }
}

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct LocalId(pub ActionHash);

impl From<ActionHash> for LocalId {
    fn from(action_hash: ActionHash) -> Self {
        LocalId(action_hash)
    }
}

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ExternalId {
    pub space_id: OutboundProxyId,
    pub local_id: LocalId,
}
impl From<(OutboundProxyId, LocalId)> for ExternalId {
    fn from(tuple: (OutboundProxyId, LocalId)) -> Self {
        ExternalId { space_id: tuple.0, local_id: tuple.1 }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropertyName(pub MapString);
impl fmt::Display for PropertyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Delegate formatting to the inner MapString
        write!(f, "{}", self.0)
    }
}
