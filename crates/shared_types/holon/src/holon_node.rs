use crate::value_types::{BaseValue, MapString};
use derive_new::new;
use hdi::prelude::*;
use std::collections::btree_map::BTreeMap;
use std::fmt;

// Define Property Related Types
pub type PropertyValue = BaseValue;
pub type PropertyMap = BTreeMap<PropertyName, PropertyValue>;
pub type SavedPropertyMap = BTreeMap<HolonId, PropertyValue>;
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropertyName(pub MapString);
impl fmt::Display for PropertyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Delegate formatting to the inner MapString
        write!(f, "{}", self.0)
    }
}

// Define Property Related Types

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub property_map: PropertyMap,
    pub saved_property_map: SavedPropertyMap,
}



#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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


impl From<(HolonSpaceId, LocalId)> for HolonId {
    fn from(tuple: (HolonSpaceId, LocalId)) -> Self {
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

impl fmt::Display for HolonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HolonId::Local(local_id) => {
                write!(f, "Local({})", local_id)
            }
            HolonId::External(external_id) => {
                write!(f, "External({}, {})", external_id.space_id, external_id.local_id)
            }
        }
    }
}

// Optionally, you can also implement Display for the related types:
impl fmt::Display for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LocalId({})", self.0)
    }
}

impl fmt::Display for HolonSpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HolonSpaceId({})", self.0)
    }
}





#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HolonSpaceId(pub ActionHash);

impl From<ActionHash> for HolonSpaceId {
    fn from(action_hash: ActionHash) -> Self {
        HolonSpaceId(action_hash)
    }
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub ActionHash);

impl From<ActionHash> for LocalId {
    fn from(action_hash: ActionHash) -> Self {
        LocalId(action_hash)
    }
}


#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExternalId {
    pub space_id : HolonSpaceId,
    pub local_id : LocalId,
}
impl From<(HolonSpaceId, LocalId)> for ExternalId {
fn from(tuple: (HolonSpaceId, LocalId)) -> Self {
    ExternalId {
        space_id: tuple.0,
        local_id: tuple.1,
    }
}
}


