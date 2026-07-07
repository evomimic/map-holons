use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceptorType {
    Local,
    Session,
    Holochain,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseReceptor {
    pub receptor_id: String,
    pub receptor_type: ReceptorType,
    pub properties: HashMap<String, String>,
}
