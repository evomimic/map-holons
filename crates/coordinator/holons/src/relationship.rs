use std::collections::BTreeMap;
use hdk::prelude::*;
use shared_types_holon::value_types::MapString;

use crate::holon_reference::HolonReference;


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq )]
pub enum RelationshipTarget {
    // Many(SmartCollection),
    ZeroOrOne(Option<HolonReference>),
    One(HolonReference),

}
pub type RelationshipName = MapString;
pub type RelationshipMap = BTreeMap<RelationshipName,RelationshipTarget>;