use async_trait::async_trait;
use core_types::{HolonError};
use std::{any::Any, fmt::Debug, sync::Arc};
use crate::shared_types::{holon_space::SpaceInfo, map_request::MapRequest, map_response::MapResponse};
use holons_core::core_shared_objects::transactions::TransactionContext;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceptorType {
    Local,
    LocalRecovery,
    Holochain,
}

//temporary hack for PR 418
impl ReceptorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReceptorType::Local => "local",
            ReceptorType::LocalRecovery => "local_recovery",
            ReceptorType::Holochain => "holochain",
        }
    }
}

//deprecated - removed in next PR
#[allow(deprecated)]
#[deprecated(note = "will be removed in favor of enum Receptor and ReceptorType")]
#[async_trait]
pub trait ReceptorBehavior: Debug + Send + Sync {
    fn transaction_context(&self) -> Arc<TransactionContext>;
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError>;
    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseReceptor {
    pub receptor_id: String,
    pub receptor_type: ReceptorType,
    #[serde(skip, default)]
    pub client_handler: Option<Arc<dyn Any + Send + Sync>>,
    pub properties: HashMap<String, String>,
}
