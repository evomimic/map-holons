use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PersistenceLinkType {
    AllHolonNodes,
    HolonNodeUpdates,
    LocalHolonSpace,
    SmartLink,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistenceLinkTag(pub Vec<u8>);
