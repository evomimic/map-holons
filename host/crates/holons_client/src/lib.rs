pub mod client_context;
pub mod client_session;
pub mod client_shared_objects;
pub mod dances_client;
pub mod deprecated_receptor_cache;
pub mod deprecated_receptor_factory;

use std::sync::Arc;

pub use client_context::{init_client_context, init_client_runtime};
pub use client_session::ClientSession;
pub use client_shared_objects::*;
use client_shared_types::{MapRequest, MapResponse, SpaceInfo};
use core_types::HolonError;
pub use holochain_receptor::DeprecatedHolochainReceptor;
use holons_core::core_shared_objects::transactions::TransactionContext;
pub use session_receptor::session_receptor::SessionReceptor;

pub enum Receptor {
    //Local(LocalReceptor),
    Holochain(DeprecatedHolochainReceptor),
    Session(SessionReceptor),
}

//these are temporarily here until transition to the map commands spec is ready
impl Receptor {
    pub fn transaction_context(&self) -> Result<Arc<TransactionContext>, HolonError> {
        match self {
            Receptor::Holochain(r) => Ok(r.transaction_context()),
            Receptor::Session(_) => Err(HolonError::NotImplemented(
                "SessionReceptor does not expose transaction_context".into(),
            )),
        }
    }

    pub async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        match self {
            Receptor::Holochain(r) => r.handle_map_request(request).await,
            Receptor::Session(_) => Err(HolonError::NotImplemented(
                "SessionReceptor does not handle map requests".into(),
            )),
        }
    }

    pub async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        match self {
            Receptor::Holochain(r) => r.get_space_info().await,
            Receptor::Session(_) => {
                Err(HolonError::NotImplemented("SessionReceptor does not expose space info".into()))
            }
        }
    }
}
