use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;

use core_types::HolonError;

use holons_client::{
    dances_client::ClientDanceBuilder,
    init_client_context,
    shared_types::{
        base_receptor::{BaseReceptor, ReceptorBehavior},
        holon_space::{HolonSpace, SpaceInfo},
        map_request::{MapRequest, MapRequestBody},
        map_response::MapResponse,
    },
};

use holons_core::dances::DanceInitiator;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_trust_channel::TrustChannel;

use crate::holochain_conductor_client::HolochainConductorClient;
use holons_loader_client::load_holons_from_files;

/// POC-safe Holochain Receptor.
/// Enough to satisfy Conductora runtime configuration.
/// Does NOT implement full space loading / root holon discovery yet.
pub struct HolochainReceptor {
    receptor_id: Option<String>,
    receptor_type: String,
    properties: HashMap<String, String>,
    context: Arc<TransactionContext>,
    client_handler: Arc<HolochainConductorClient>,
    _home_space_holon: HolonSpace,
}

impl HolochainReceptor {
    pub fn new(base: BaseReceptor) -> Self {
        // Downcast the stored client into our concrete conductor client
        let client_any =
            base.client_handler.as_ref().expect("Client is required for HolochainReceptor").clone();

        let client_handler = client_any
            .downcast::<HolochainConductorClient>()
            .expect("Failed to downcast client to HolochainConductorClient");

        // Trust channel wraps the conductor client
        let trust_channel = TrustChannel::new(client_handler.clone());
        let initiator: Arc<dyn DanceInitiator + Send + Sync> = Arc::new(trust_channel);

        // Build client context with dance initiator
        let context = init_client_context(Some(initiator));

        // Default until we fully implement space discovery
        let _home_space_holon = HolonSpace::default();

        Self {
            receptor_id: base.receptor_id.clone(),
            receptor_type: base.receptor_type.clone(),
            properties: base.properties.clone(),
            context,
            client_handler,
            _home_space_holon,
        }
    }
}

#[async_trait]
impl ReceptorBehavior for HolochainReceptor {
    fn transaction_context(&self) -> Arc<TransactionContext> {
        Arc::clone(&self.context)
    }

    /// Core request → client dance pipeline
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        let dance_request =
            ClientDanceBuilder::validate_and_execute(self.context.as_ref(), &request)?;

        let initiator =
            self.context.get_dance_initiator().expect("Dance initiator must be initialized");

        let dance_response = initiator.initiate_dance(&*self.context, dance_request).await;

        Ok(MapResponse::new_from_dance_response(request.space.id, dance_response))
    }

    /// POC stub for system info
    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        // Call stubbed conductor client
        self.client_handler.get_all_spaces().await
    }

    //todo: integrate this into the map_request handling flow,  this is a PoC hack
    async fn load_holons(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        if let MapRequestBody::LoadHolons(content_set) = request.body {
            let reference = load_holons_from_files(self.context.clone(), content_set).await?;
            tracing::info!("HolochainReceptor: loaded holons with reference: {:?}", reference);

            //temporary hack to get a DanceResponse after loading holons
            let dance_request = ClientDanceBuilder::get_all_holons_dance()?; //self.context.as_ref(), &request)?;
            let initiator =
                self.context.get_dance_initiator().expect("Dance initiator must be initialized");
            let dance_response = initiator.initiate_dance(&*self.context, dance_request).await;
            Ok(MapResponse::new_from_dance_response(request.space.id, dance_response))
        } else {
            Err(HolonError::InvalidParameter(
                "Expected LoadHolons body for load_holons request".into(),
            ))
        }
    }
}

impl Debug for HolochainReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HolochainReceptor")
            .field("receptor_id", &self.receptor_id)
            .field("receptor_type", &self.receptor_type)
            .field("properties", &self.properties)
            .finish()
    }
}
