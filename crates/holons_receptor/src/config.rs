use async_trait::async_trait;
use core_types::HolonError;
use holochain_receptor::HolochainReceptor;
use holons_client::shared_types::{
    holon_space::SpaceInfo,
    map_request::MapRequest,
    map_response::MapResponse,
    base_receptor::ReceptorBehavior,
};
use crate::local_receptor::LocalReceptor;

#[derive(Debug)]
pub enum Receptor {
    Holochain(HolochainReceptor),
    Local(LocalReceptor),
}

#[async_trait]
impl ReceptorBehavior for Receptor {
    async fn handle_map_request(
        &self,
        request: MapRequest,
    ) -> Result<MapResponse, HolonError> {

        match self {
            Receptor::Holochain(h) => h.handle_map_request(request).await,
            Receptor::Local(l) => l.handle_map_request(request).await,
        }
    }

    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        match self {
            Receptor::Holochain(h) => h.get_space_info().await,
            Receptor::Local(l) => l.get_space_info().await,
        }
    }
}