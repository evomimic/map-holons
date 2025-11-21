use core_types::HolonError;
use holons_client::shared_types::holon_space::{HolonSpace, SpaceInfo};
use holons_client::shared_types::base_receptor::{BaseReceptor, Receptor as ReceptorTrait};
use holons_client::{ClientHolonService, init_client_context};
use holons_client::shared_types::map_request::MapRequest;
use holons_client::shared_types::map_response::MapResponse;
use holons_core::HolonServiceApi;
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use holons_core::reference_layer::HolonsContextBehavior;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use async_trait::async_trait;
use crate::{LocalClient};

pub const ROOT_SPACE_HOLON_PATH: &str = "root_holon_space";
pub const ROOT_HOLON_SPACE_NAME: &str = "RootHolonSpace";
pub const ROOT_HOLON_SPACE_DESCRIPTION: &str = "Default Root Holon Space";

pub struct LocalReceptor {
    receptor_id: Option<String>,
    receptor_type: String,
    properties: HashMap<String, String>,
    context: Arc<dyn HolonsContextBehavior + Send + Sync>,
    client_handler: Arc<LocalClient>,
    _holon_service: Arc<dyn HolonServiceApi>,
    _root_space: HolonSpace,
}

/// Implementation of LocalReceptor - local host level - no dancing
impl LocalReceptor {
    /// Create a new LocalReceptor, returning Result to handle downcast failures
    pub fn new(base_receptor: BaseReceptor) -> Result<Self, HolonError> {

        let context = init_client_context(None);
        
        //ENSURE ROOT HOLON EXISTS OR CREATE IT
        let client = LocalClient::new();
       // let holon = client.fetch_or_create_root_holon(context.as_ref())?;
       // let root_space = client.convert_to_holonspace(holon)?;
        let client_handler = Arc::new(client);

        let _holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);
        
        Ok(Self {
            receptor_id: base_receptor.receptor_id.clone(),
            receptor_type: base_receptor.receptor_type.clone(),
            properties: base_receptor.properties.clone(),
            context,
            client_handler,
            _holon_service,
           _root_space: HolonSpace::default(), //root_space,
        })
    }
}

#[async_trait]
impl ReceptorTrait for LocalReceptor {
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        tracing::warn!("LocalReceptor: handling request: {:?}", self.context);
        
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

//is still needed?
impl fmt::Debug for LocalReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalReceptor")
            .field("receptor_id", &self.receptor_id)
            .field("receptor_type", &self.receptor_type)
            .field("properties", &self.properties)
            .field("context", &"ClientHolonsContext")
           // .field("root_space", &self.root_space)
            .finish()
    }
}