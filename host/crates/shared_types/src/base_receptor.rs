use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{any::Any, fmt::Debug, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceptorType {
    Local,
    LocalRecovery,
    Holochain,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseReceptor {
    pub receptor_id: String,
    pub receptor_type: ReceptorType,
    #[serde(skip, default)]
    pub client_handler: Option<Arc<dyn Any + Send + Sync>>,
    pub properties: HashMap<String, String>,
}
