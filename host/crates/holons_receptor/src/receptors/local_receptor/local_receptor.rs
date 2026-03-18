use core_types::{HolonError};
use holons_client::client_context::ClientSession;
use holons_client::shared_types::holon_space::{HolonSpace, SpaceInfo};
use holons_client::shared_types::base_receptor::{BaseReceptor, ReceptorBehavior};
use holons_client::{ClientHolonService, init_client_context};
use holons_client::shared_types::map_request::MapRequest;
use holons_client::shared_types::map_response::MapResponse;
use holons_core::HolonServiceApi;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use std::fmt;
use std::fmt::Debug;
use async_trait::async_trait;
use crate::{LocalClient};

pub const ROOT_SPACE_HOLON_PATH: &str = "root_holon_space";
pub const ROOT_HOLON_SPACE_NAME: &str = "RootHolonSpace";
pub const ROOT_HOLON_SPACE_DESCRIPTION: &str = "Default Root Holon Space";

pub struct LocalReceptor {
    _receptor_id: Option<String>,
    _receptor_type: String,
    _properties: HashMap<String, String>,
    session: ClientSession,
    client_handler: Arc<LocalClient>,
    _holon_service: Arc<dyn HolonServiceApi>,
    _root_space: HolonSpace,
}

/// Implementation of LocalReceptor - local host level - no dancing
impl LocalReceptor {
    /// Create a new LocalReceptor, returning Result to handle downcast failures
    pub fn new(base: BaseReceptor) -> Result<Self, HolonError> {

        // Build client context with dance initiator and recovery store if provided
        let session = if let Some(recovery_store) = base.snapshot_store.as_ref() {
            init_client_context(None, Some(Arc::clone(recovery_store)))
        } else {
            init_client_context(None, None)
        };
        
        //ENSURE ROOT HOLON EXISTS OR CREATE IT
        let client = LocalClient::new();
       // let holon = client.fetch_or_create_root_holon(context.as_ref())?;
       // let root_space = client.convert_to_holonspace(holon)?;
        let client_handler = Arc::new(client);

        let _holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);
        
        Ok(Self {
            _receptor_id: base.receptor_id.clone(),
            _receptor_type: base.receptor_type.clone(),
            _properties: base.properties.clone(),
            session,
            client_handler,
            _holon_service,
           _root_space: HolonSpace::default(), //root_space,
        })
    }
}

#[async_trait]
impl ReceptorBehavior for LocalReceptor {
    fn transaction_context(&self) -> Arc<TransactionContext> {
        Arc::clone(&self.session.context)
    }

    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        tracing::warn!("LocalReceptor: handling request: {:?}", self.session.context);
        
        //TODO: implement actual handling logic here with the HolonServiceApi

        let mocked_response = MapResponse {
            space_id: request.space.id,
            status_code: ResponseStatusCode::OK,
            description: "Local request completed".into(),
            body: ResponseBody::None,
            descriptor: None,
            state: None,
        };

        tracing::info!("LocalReceptor: request response: {:?}", mocked_response);

        //let res = MapResponse::new_from_dance_response(request.space.id, moocked_response);
        Ok(mocked_response)
    }
    //async fn add_space(&self, holon_for_space: Holon) -> Result<(), HolonError> {

    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        self.client_handler.get_all_spaces().await
    }
}

impl Debug for LocalReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalReceptor")
            .field("receptor_id", &self._receptor_id)
            .field("receptor_type", &self._receptor_type)
            .field("properties", &self._properties)
            .finish()
    }
}
