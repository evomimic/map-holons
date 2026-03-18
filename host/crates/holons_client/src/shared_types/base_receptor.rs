use async_trait::async_trait;
use core_types::{HolonError};
use std::{any::Any, fmt, sync::Arc};
use crate::{shared_types::{holon_space::SpaceInfo, map_request::MapRequest, map_response::MapResponse}};
use holons_core::core_shared_objects::transactions::TransactionContext;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait ReceptorBehavior: Send + Sync {
    fn transaction_context(&self) -> Arc<TransactionContext>;
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError>;
    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError>;
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BaseReceptor {
    pub receptor_id: Option<String>,
    pub receptor_type: String,
    #[serde(skip, default)]
    pub client_handler: Option<Arc<dyn Any + Send + Sync>>,
    #[serde(skip, default)]
    pub snapshot_store: Option<Arc<dyn Any + Send + Sync>>,
    pub properties: HashMap<String, String>,
}

/// Dispatching debug message for receptor
impl fmt::Debug for BaseReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BaseReceptor")
            .field("receptor_id", &self.receptor_id)
            .field("receptor_type", &self.receptor_type)
            .field("properties", &self.properties)
            .finish()
    }
}
