use async_trait::async_trait;
use core_types::HolonError;
use holochain_receptor::{HolochainReceptor};
use holons_client::shared_types::{holon_space::SpaceInfo, map_request::MapRequest, map_response::MapResponse, base_receptor::Receptor as ReceptorBehavior};
use crate::local_receptor::LocalReceptor;

#[derive(Debug)]
pub enum Receptor {
    Holochain(HolochainReceptor),
    Local(LocalReceptor),
    // Add other receptor types here
}

#[async_trait]
impl ReceptorBehavior for Receptor {
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        match self {
            Receptor::Holochain(h) => ReceptorBehavior::handle_map_request(h, request).await,
            Receptor::Local(l) => ReceptorBehavior::handle_map_request(l, request).await,
        }
    }
    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        match self {
            Receptor::Holochain(h) => ReceptorBehavior::get_space_info(h).await,
            Receptor::Local(l) => ReceptorBehavior::get_space_info(l).await,
        }
    }
}

