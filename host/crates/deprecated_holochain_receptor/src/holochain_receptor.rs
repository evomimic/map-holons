use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use crate::holochain_conductor_client::HolochainConductorClient;
use base_types::MapString;
use core_types::HolonError;
use crate::dances_client::ClientDanceBuilder;
use crate::client_context::init_client_context;
use client_shared_types::{
        base_receptor::{BaseReceptor, ReceptorType},
        holon_space::{HolonSpace, SpaceInfo},
        map_request::{MapRequest, MapRequestBody},
        map_response::MapResponse
};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceInitiator, DanceResponse, ResponseBody, ResponseStatusCode};
use holons_core::reference_layer::HolonReference;
use holons_loader_client::load_holons_from_files;
use holons_trust_channel::TrustChannel;

/// POC-safe Holochain Receptor.
/// Enough to satisfy Conductora runtime configuration.
/// Does NOT implement full space loading / root holon discovery yet.
pub struct HolochainReceptor {
    receptor_id: String,
    receptor_type: ReceptorType,
    properties: HashMap<String, String>,
    context: Arc<TransactionContext>,
    client_handler: Arc<HolochainConductorClient>,
    _home_space_holon: HolonSpace,
}

impl HolochainReceptor {
    fn is_commit_dance_request(request_name: &str) -> bool {
        matches!(request_name, "commit")
    }

    fn is_read_only_request(request_name: &str) -> bool {
        matches!(request_name, "get_all_holons" | "get_holon_by_id" | "query_relationships")
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
            receptor_type: base.receptor_type,
            properties: base.properties.clone(),
            context,
            client_handler,
            _home_space_holon,
        }
    }
}

impl HolochainReceptor {
    pub fn transaction_context(&self) -> Arc<TransactionContext> {
        Arc::clone(&self.context)
    }

    /// Core request → client dance pipeline
    pub async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        // Temporary Phase 1.4/1.5 bridge: commit-like requests serialize host
        // ingress here. In Phase 2 this moves to CommandDispatcher.
        if Self::is_commit_dance_request(request.name.as_str()) {
            let _commit_guard = self.context.begin_host_commit_ingress_guard()?;
            // Preserve request-shape validation before routing to context-owned commit execution.
            let _validated_request = ClientDanceBuilder::validate_and_execute(&self.context, &request)?;
            let response_reference = self.context.commit()?;
            let dance_response = DanceResponse::new(
                ResponseStatusCode::OK,
                MapString("Commit executed via TransactionContext".to_string()),
                ResponseBody::HolonReference(HolonReference::Transient(response_reference)),
                None,
            );

            return Ok(MapResponse::new_from_dance_response(request.space.id, dance_response));
        }

        // Temporary Phase 1.4/1.5 bridge: load+commit is commit-like and
        // should serialize at host ingress until CommandDispatcher owns this.
        if request.name == "load_holons" {
            let _commit_guard = self.context.begin_host_commit_ingress_guard()?;

            let content_set = match request.body {
                MapRequestBody::LoadHolons(content_set) => content_set,
                _ => {
                    return Err(HolonError::InvalidParameter(
                        "Expected LoadHolons body for load_holons request".into(),
                    ))
                }
            };

            let response_reference = load_holons_from_files(self.context.clone(), content_set).await?;
            tracing::info!(
                "HolochainReceptor: loaded holons with reference: {:?}",
                response_reference
            );

            let dance_response = DanceResponse::new(
                ResponseStatusCode::OK,
                MapString("LoadHolons executed via TransactionContext".to_string()),
                ResponseBody::HolonReference(HolonReference::Transient(response_reference)),
                None,
            );

            return Ok(MapResponse::new_from_dance_response(request.space.id, dance_response));
        }

        // Read/query requests remain available during host commit ingress and after
        // lifecycle reaches Committed so clients can inspect commit/load results.
        // External write/mutation requests (including transient creation) require
        // an open transaction and must be blocked during host commit ingress.
        let is_read_only = Self::is_read_only_request(request.name.as_str());
        if !is_read_only {
            self.context.ensure_host_mutation_entry_allowed()?;
        }

        let dance_request = ClientDanceBuilder::validate_and_execute(&self.context, &request)?;
        let dance_response = self
            .context
            .initiate_ingress_dance(dance_request, is_read_only)
            .await?;

        Ok(MapResponse::new_from_dance_response(request.space.id, dance_response))
    }

    /// POC stub for system info
   pub async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
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

#[cfg(test)]
mod tests {
    use super::HolochainReceptor;

    #[test]
    fn commit_route_classification_is_exact() {
        assert!(HolochainReceptor::is_commit_dance_request("commit"));
        assert!(!HolochainReceptor::is_commit_dance_request("get_all_holons"));
        assert!(!HolochainReceptor::is_commit_dance_request("load_holons"));
    }

    #[test]
    fn read_only_route_classification_includes_supported_reads() {
        assert!(HolochainReceptor::is_read_only_request("get_all_holons"));
        assert!(HolochainReceptor::is_read_only_request("get_holon_by_id"));
        assert!(HolochainReceptor::is_read_only_request("query_relationships"));
    }

    #[test]
    fn read_only_route_classification_excludes_mutations() {
        assert!(!HolochainReceptor::is_read_only_request("commit"));
        assert!(!HolochainReceptor::is_read_only_request("create_new_holon"));
        assert!(!HolochainReceptor::is_read_only_request("stage_new_holon"));
        assert!(!HolochainReceptor::is_read_only_request("load_holons"));
    }
}
