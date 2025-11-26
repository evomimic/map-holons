use async_trait::async_trait;
use core_types::HolonError;

use std::{fmt::Debug, sync::Arc, any::Any};
use crate::shared_types::{holon_space::SpaceInfo, map_request::MapRequest, map_response::MapResponse};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};



#[async_trait]
pub trait ReceptorBehavior: Debug + Send + Sync {
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

