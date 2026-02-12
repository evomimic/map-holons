use async_trait::async_trait;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceRequest, DanceResponse};
use std::sync::Arc;
use tracing::debug;

use crate::dance_envelope_transport::DanceEnvelopeTransport;
use crate::envelopes::dance_envelope_adapter::DanceEnvelopeAdapter;
use holons_core::dances::dance_initiator::DanceInitiator;

/// The TrustChannel coordinates envelope flow for outbound and inbound Dances.
///
/// It wraps an inner envelope transport backend and applies runtime <-> envelope
/// conversion before and after the core invocation.
#[derive(Debug, Clone)]
pub struct TrustChannel {
    backend: std::sync::Arc<dyn DanceEnvelopeTransport + Send + Sync>,
}

impl TrustChannel {
    /// Constructs a new TrustChannel around a backend envelope transport.
    pub fn new(backend: std::sync::Arc<dyn DanceEnvelopeTransport + Send + Sync>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl DanceInitiator for TrustChannel {
    async fn initiate_dance(
        &self,
        context: &Arc<TransactionContext>,
        request: DanceRequest,
    ) -> DanceResponse {
        // --- Outbound runtime -> envelope -----------------------------------------
        let request_envelope = match DanceEnvelopeAdapter::build_request_envelope(&context, request)
        {
            Ok(envelope) => envelope,
            Err(error) => return DanceResponse::from_error(error),
        };

        debug!("TrustChannel::initiate_dance() — prepared envelope request");

        // --- Transmit via backend --------------------------------------------
        let response_envelope = match self.backend.initiate_dance_envelope(request_envelope).await {
            Ok(envelope) => envelope,
            Err(error) => return DanceResponse::from_error(error),
        };

        // --- Inbound envelope -> runtime -------------------------------------
        let response = match DanceEnvelopeAdapter::bind_response_envelope(&context, response_envelope)
        {
            Ok(response) => response,
            Err(error) => return DanceResponse::from_error(error),
        };

        debug!("TrustChannel::initiate_dance() — got response: {:?}", response.summarize());
        response
    }
}
