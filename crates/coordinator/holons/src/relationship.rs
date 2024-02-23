use hdk::prelude::*;
use shared_types_holon::value_types::MapString;
use std::collections::BTreeMap;

use crate::staged_reference::StagedReference;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RelationshipTarget {
    // Many(SmartCollection),
    ZeroOrOne(Option<StagedReference>),
    One(StagedReference),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationshipName(pub MapString);

pub type RelationshipMap = BTreeMap<RelationshipName, RelationshipTarget>;
