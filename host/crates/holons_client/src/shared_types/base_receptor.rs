use crate::shared_types::{
    holon_space::SpaceInfo, map_request::MapRequest, map_response::MapResponse,
};
use async_trait::async_trait;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{any::Any, fmt::Debug, sync::Arc};

#[async_trait]
pub trait ReceptorBehavior: Debug + Send + Sync {
    fn transaction_context(&self) -> Arc<TransactionContext>;
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError>;
    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseReceptor {
    pub receptor_id: Option<String>,
    pub receptor_type: String,
    #[serde(skip, default)]
    pub client_handler: Option<Arc<dyn Any + Send + Sync>>,
    pub properties: HashMap<String, String>,
}
