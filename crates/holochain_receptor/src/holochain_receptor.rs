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
        map_request::MapRequest,
        map_response::MapResponse,
    },
};

use holons_core::{dances::DanceInitiator, HolonsContextBehavior};
use holons_trust_channel::TrustChannel;

use crate::holochain_conductor_client::HolochainConductorClient;

/// POC-safe Holochain Receptor.
/// Enough to satisfy Conductora runtime configuration.
/// Does NOT implement full space loading / root holon discovery yet.
pub struct HolochainReceptor {
    receptor_id: Option<String>,
    receptor_type: String,
    properties: HashMap<String, String>,
    context: Arc<dyn HolonsContextBehavior + Send + Sync>,
    client_handler: Arc<HolochainConductorClient>,
    home_space_holon: HolonSpace,
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
        let home_space_holon = HolonSpace::default();

        Self {
            receptor_id: base.receptor_id.clone(),
            receptor_type: base.receptor_type.clone(),
            properties: base.properties.clone(),
            context,
            client_handler,
            home_space_holon,
        }
    }
}

#[async_trait]
impl ReceptorBehavior for HolochainReceptor {
    /// Core request → client dance pipeline
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        let dance_request =
            ClientDanceBuilder::validate_and_execute(self.context.as_ref(), &request)?;

        let initiator = self
            .context
            .get_space_manager()
            .get_dance_initiator()
            .expect("Dance initiator must be initialized");

        let dance_response = initiator.initiate_dance(&*self.context, dance_request).await;

        Ok(MapResponse::new_from_dance_response(request.space.id, dance_response))
    }

    /// POC stub for system info
    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        // Call stubbed conductor client
        self.client_handler.get_all_spaces().await
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
