use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use base_types::{BaseValue, MapString};

use core_types::{HolonError, PropertyName};

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

use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceInitiator, ResponseStatusCode};
use holons_core::reference_layer::{ReadableHolon, TransientReference};
use holons_core::HolonsContextBehavior;
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
    fn is_commit_request(request_name: &str) -> bool {
        matches!(request_name, "commit" | "load_holons")
    }

    fn is_read_only_request(request_name: &str) -> bool {
        matches!(request_name, "get_all_holons" | "get_holon_by_id" | "query_relationships")
    }

    fn is_transient_only_request(request_name: &str) -> bool {
        matches!(request_name, "create_new_holon")
    }

    fn enforce_lifecycle_for_request(&self, request_name: &str) -> Result<(), HolonError> {
        if Self::is_commit_request(request_name) {
            return self.context.ensure_commit_allowed();
        }

        if Self::is_read_only_request(request_name) {
            return Ok(());
        }

        if Self::is_transient_only_request(request_name) {
            if self.context.is_commit_in_progress() {
                return Err(HolonError::TransactionCommitInProgress {
                    tx_id: self.context.tx_id().value(),
                });
            }
            return Ok(());
        }

        self.context.ensure_open_for_external_mutation()
    }

    fn should_transition_from_load_response(
        load_response_reference: &TransientReference,
    ) -> Result<bool, HolonError> {
        let load_commit_status_property = PropertyName(MapString("LoadCommitStatus".to_string()));
        let status_value = load_response_reference.property_value(load_commit_status_property)?;

        match status_value {
            Some(BaseValue::StringValue(status)) => match status.0.as_str() {
                "Complete" => Ok(true),
                "Incomplete" | "Skipped" => Ok(false),
                other => Err(HolonError::InvalidParameter(format!(
                    "Unexpected LoadCommitStatus value on HolonLoadResponse: {}",
                    other
                ))),
            },
            Some(other) => Err(HolonError::InvalidType(format!(
                "LoadCommitStatus on HolonLoadResponse must be a StringValue, found {:?}",
                other
            ))),
            None => Ok(false),
        }
    }

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
        self.enforce_lifecycle_for_request(request.name.as_str())?;

        let dance_request = ClientDanceBuilder::validate_and_execute(&self.context, &request)?;

        let initiator = self.context.get_dance_initiator()?;

        if Self::is_commit_request(request.name.as_str()) {
            if !self.context.try_begin_commit() {
                return Err(HolonError::TransactionCommitInProgress {
                    tx_id: self.context.tx_id().value(),
                });
            }

            let dance_response = initiator.initiate_dance(&self.context, dance_request).await;

            // Keep the execution guard held until lifecycle transition is finalized.
            let transition_result = if dance_response.status_code == ResponseStatusCode::OK {
                self.context.transition_to_committed()
            } else {
                Ok(())
            };
            self.context.end_commit();
            transition_result?;

            return Ok(MapResponse::new_from_dance_response(request.space.id, dance_response));
        }

        let dance_response = initiator.initiate_dance(&self.context, dance_request).await;

        Ok(MapResponse::new_from_dance_response(request.space.id, dance_response))
    }

    /// POC stub for system info
    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        // Call stubbed conductor client
        self.client_handler.get_all_spaces().await
    }

    //todo: integrate this into the map_request handling flow,  this is a PoC hack
    async fn load_holons(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        self.enforce_lifecycle_for_request(request.name.as_str())?;

        if !self.context.try_begin_commit() {
            return Err(HolonError::TransactionCommitInProgress {
                tx_id: self.context.tx_id().value(),
            });
        }

        let result = if let MapRequestBody::LoadHolons(content_set) = request.body {
            let reference = load_holons_from_files(self.context.clone(), content_set).await?;
            tracing::info!("HolochainReceptor: loaded holons with reference: {:?}", reference);

            if Self::should_transition_from_load_response(&reference)? {
                self.context.transition_to_committed()?;
            }

            let dance_request = ClientDanceBuilder::get_all_holons_dance()?;
            let initiator = self.context.get_dance_initiator()?;
            let dance_response = initiator.initiate_dance(&self.context, dance_request).await;
            Ok(MapResponse::new_from_dance_response(request.space.id, dance_response))
        } else {
            Err(HolonError::InvalidParameter(
                "Expected LoadHolons body for load_holons request".into(),
            ))
        };

        self.context.end_commit();
        result
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
