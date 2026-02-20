use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use base_types::BaseValue;

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

use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceInitiator, DanceResponse, ResponseBody, ResponseStatusCode};
use holons_core::reference_layer::{HolonReference, ReadableHolon, TransientReference};
use holons_core::HolonsContextBehavior;
use holons_trust_channel::TrustChannel;
use type_names::CorePropertyTypeName;

use crate::holochain_conductor_client::HolochainConductorClient;
use holons_loader_client::load_holons_from_files;

fn finalize_commit_transition(
    context: &Arc<TransactionContext>,
    should_transition_to_committed: bool,
) -> Result<(), HolonError> {
    if should_transition_to_committed {
        context.transition_to_committed()?;
    }

    Ok(())
}

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
        // Should hardcode these names if we want to keep this pattern of lifecycle enforcement based on request type
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
            if self.context.is_host_commit_in_progress() {
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
        let status_value =
            load_response_reference.property_value(CorePropertyTypeName::LoadCommitStatus)?;

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

    fn should_transition_from_commit_response(
        dance_response: &DanceResponse,
    ) -> Result<bool, HolonError> {
        let commit_response_reference = match &dance_response.body {
            ResponseBody::HolonReference(HolonReference::Transient(reference)) => reference,
            ResponseBody::HolonReference(other) => {
                return Err(HolonError::InvalidType(format!(
                    "Expected commit response to return TransientReference, found {:?}",
                    other
                )))
            }
            other => {
                return Err(HolonError::InvalidParameter(format!(
                    "Expected commit response body to be HolonReference, found {:?}",
                    other
                )))
            }
        };

        let status_value =
            commit_response_reference.property_value(CorePropertyTypeName::CommitRequestStatus)?;

        match status_value {
            Some(BaseValue::StringValue(status)) => match status.0.as_str() {
                "Complete" => Ok(true),
                "Incomplete" => Ok(false),
                other => Err(HolonError::InvalidParameter(format!(
                    "Unexpected CommitRequestStatus value on CommitResponse: {}",
                    other
                ))),
            },
            Some(other) => Err(HolonError::InvalidType(format!(
                "CommitRequestStatus on CommitResponse must be a StringValue, found {:?}",
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
            let _commit_guard = self.context.begin_host_commit_ingress_guard()?;

            let dance_response = initiator.initiate_dance(&self.context, dance_request).await;

            // Keep the execution guard held until lifecycle transition is finalized.
            let should_transition_to_committed = dance_response.status_code
                == ResponseStatusCode::OK
                && Self::should_transition_from_commit_response(&dance_response)?;

            finalize_commit_transition(&self.context, should_transition_to_committed)?;

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

        let _commit_guard = self.context.begin_host_commit_ingress_guard()?;

        let result = if let MapRequestBody::LoadHolons(content_set) = request.body {
            let reference = load_holons_from_files(self.context.clone(), content_set).await?;
            tracing::info!("HolochainReceptor: loaded holons with reference: {:?}", reference);

            let should_transition_to_committed =
                Self::should_transition_from_load_response(&reference)?;
            finalize_commit_transition(&self.context, should_transition_to_committed)?;

            let dance_request = ClientDanceBuilder::get_all_holons_dance()?;
            let initiator = self.context.get_dance_initiator()?;
            let dance_response = initiator.initiate_dance(&self.context, dance_request).await;
            Ok(MapResponse::new_from_dance_response(request.space.id, dance_response))
        } else {
            Err(HolonError::InvalidParameter(
                "Expected LoadHolons body for load_holons request".into(),
            ))
        };

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

#[cfg(test)]
mod tests {
    use super::finalize_commit_transition;
    use core_types::HolonError;
    use holons_client::init_client_context;
    use holons_core::core_shared_objects::transactions::TransactionLifecycleState;

    #[test]
    fn commit_execution_guard_sets_and_releases_flag() {
        let context = init_client_context(None);
        assert!(!context.is_host_commit_in_progress());

        {
            let _guard = context
                .begin_host_commit_ingress_guard()
                .expect("guard acquisition should succeed");
            assert!(context.is_host_commit_in_progress());
        }

        assert!(!context.is_host_commit_in_progress());
    }

    #[test]
    fn commit_execution_guard_rejects_reentrant_acquire() {
        let context = init_client_context(None);
        let _guard = context
            .begin_host_commit_ingress_guard()
            .expect("first acquisition should succeed");

        let err = context
            .begin_host_commit_ingress_guard()
            .expect_err("second acquisition while held should fail");

        assert!(matches!(err, HolonError::TransactionCommitInProgress { .. }));
    }

    #[test]
    fn commit_execution_guard_releases_on_early_error_path() {
        let context = init_client_context(None);

        let result: Result<(), HolonError> = (|| {
            let _guard = context.begin_host_commit_ingress_guard()?;
            Err(HolonError::InvalidParameter("synthetic failure".into()))
        })();

        assert!(matches!(result, Err(HolonError::InvalidParameter(_))));
        assert!(
            !context.is_host_commit_in_progress(),
            "guard must be released even when scope exits through error"
        );
    }

    #[test]
    fn finalize_commit_transition_applies_open_to_committed_only_when_requested() {
        let context = init_client_context(None);
        assert_eq!(context.lifecycle_state(), TransactionLifecycleState::Open);

        finalize_commit_transition(&context, false).expect("no-op finalize should succeed");
        assert_eq!(context.lifecycle_state(), TransactionLifecycleState::Open);

        finalize_commit_transition(&context, true).expect("transition finalize should succeed");
        assert_eq!(context.lifecycle_state(), TransactionLifecycleState::Committed);
    }

    #[test]
    fn finalize_commit_transition_rejects_double_commit_transition() {
        let context = init_client_context(None);

        finalize_commit_transition(&context, true).expect("first transition should succeed");

        let err = finalize_commit_transition(&context, true)
            .expect_err("second transition attempt should fail deterministically");

        assert!(matches!(err, HolonError::TransactionAlreadyCommitted { .. }));
    }
}
