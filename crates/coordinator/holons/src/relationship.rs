use hdk::prelude::*;
use shared_types_holon::value_types::MapString;
use std::collections::BTreeMap;

use crate::holon_reference::HolonReference;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RelationshipTarget {
    // Many(SmartCollection),
    ZeroOrOne(Option<HolonReference>),
    One(HolonReference),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationshipName(pub MapString);

pub type RelationshipMap = BTreeMap<RelationshipName, RelationshipTarget>;
