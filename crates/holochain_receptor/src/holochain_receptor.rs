use crate::holochain_conductor_client::HolochainConductorClient;
use core_types::{HolonError};
use holons_client::dances_client::ClientDanceBuilder;
use holons_client::shared_types::holon_space::{HolonSpace, SpaceInfo};
use holons_client::shared_types::map_response::MapResponse;
use holons_client::shared_types::base_receptor::{BaseReceptor, Receptor};
use holons_client::{init_client_context};
use holons_client::shared_types::map_request::{MapRequest};
use holons_core::dances::{DanceInitiator};
use holons_core::reference_layer::HolonsContextBehavior;
use holons_trust_channel::TrustChannel;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use async_trait::async_trait;


pub struct HolochainReceptor {
    receptor_id: Option<String>,
    receptor_type: String,
    properties: HashMap<String, String>,
    context: Arc<dyn HolonsContextBehavior + Send + Sync>,
    client_handler: Arc<HolochainConductorClient>,
    home_space_holon: HolonSpace,
}

impl HolochainReceptor {
    pub fn new(base_receptor: BaseReceptor) -> Self {
        // Downcast Arc<dyn Any> to Arc<HolochainClient>
        let client_any = base_receptor.client_handler.as_ref()
            .expect("Client is required for HolochainReceptor")
            .clone();
        
        let client_handler = client_any
            .downcast::<HolochainConductorClient>()
            .expect("Failed to downcast client to HolochainClient");

        let trust_channel = TrustChannel::new(client_handler.clone());
        let dance_initiator: Arc<dyn DanceInitiator + Send + Sync> = Arc::new(trust_channel);
        let context = init_client_context(Some(dance_initiator.clone()));

        //TODO: obtain locally or fetch home space holon from holochain client
        let home_space_holon = HolonSpace::default();

        Self {
            receptor_id: base_receptor.receptor_id.clone(),
            receptor_type: base_receptor.receptor_type.clone(),
            properties: base_receptor.properties.clone(),
            context,
            client_handler,
            home_space_holon
        }
    }
}

//this is temporarily the way to make dance requests until we have a full dance service implementation
#[async_trait]
impl Receptor for HolochainReceptor {
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {  
        tracing::info!("HolochainReceptor: handling map request: {:?}", request);
        let dance_request = ClientDanceBuilder::validate_and_execute(self.context.as_ref(), &request)?;
            
        tracing::info!("HolochainReceptor: handling dance request: {:?}", dance_request);
        let initiator = self
            .context
            .get_space_manager()
            .get_dance_initiator()
            .expect("Dance initiator must be initialized in test context");
        // Call the pipeline — always returns a DanceResponse
        let dance_response = initiator.initiate_dance(&*self.context, dance_request).await;
        let map_response = MapResponse::new_from_dance_response(request.space.id, dance_response);
        Ok(map_response)
    }

    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        self.client_handler.get_all_spaces().await
    }

    

}

impl fmt::Debug for HolochainReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HolochainReceptor")
            .field("dance_service", &"HolochainDanceService")
            .field("context", &"ClientHolonsContext")
            .field("home_space_holon", &self.home_space_holon)
            .field("receptor_id", &self.receptor_id)
            .field("receptor_type", &self.receptor_type)
            .field("properties", &self.properties)
            .finish()
    }
}