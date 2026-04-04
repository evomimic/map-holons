pub mod client_context;
pub mod client_session;
pub mod client_shared_objects;
pub mod dances_client;
pub mod receptor_factory;
pub mod receptor_cache;

use std::sync::Arc;

pub use client_context::{init_client_context, init_client_runtime};
pub use client_session::ClientSession;
pub use client_shared_objects::*;
use client_shared_types::{MapRequest, MapResponse, SpaceInfo};
use core_types::HolonError;
pub use deprecated_holochain_receptor::HolochainReceptor;
use holons_core::core_shared_objects::transactions::TransactionContext;
pub use recovery_receptor::local_recovery_receptor::LocalRecoveryReceptor;

pub enum Receptor {
    //Local(LocalReceptor),
    Holochain(HolochainReceptor),
    LocalRecovery(LocalRecoveryReceptor),
}

//these are temporarily here until transition to the map commands spec is ready
impl Receptor {
    pub fn transaction_context(&self) -> Result<Arc<TransactionContext>, HolonError> {
        match self {
            Receptor::Holochain(r) => Ok(r.transaction_context()),
            Receptor::LocalRecovery(_) => Err(HolonError::NotImplemented(
                "LocalRecoveryReceptor does not expose transaction_context".into()
            )),
        }
    }

    pub async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        match self {
            Receptor::Holochain(r) => r.handle_map_request(request).await,
            Receptor::LocalRecovery(_) => Err(HolonError::NotImplemented(
                "LocalRecoveryReceptor does not handle map requests".into(),
            )),
        }
    }

    pub async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        match self {
            Receptor::Holochain(r) => r.get_space_info().await,
            Receptor::LocalRecovery(_) => Err(HolonError::NotImplemented(
                "LocalRecoveryReceptor does not expose space info".into(),
            )),
        }
    }
}
