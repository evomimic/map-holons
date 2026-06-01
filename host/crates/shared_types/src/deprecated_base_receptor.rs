use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{any::Any, fmt::Debug, sync::Arc};

use crate::ReceptorType;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeprecatedBaseReceptor {
    pub receptor_id: String,
    pub receptor_type: ReceptorType,
    #[serde(skip, default)]
    pub client_handler: Option<Arc<dyn Any + Send + Sync>>,
    pub properties: HashMap<String, String>,
}
