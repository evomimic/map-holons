use async_trait::async_trait;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceRequest, DanceResponse};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info};

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
        let is_load_holons = request.dance_name.0 == "load_holons";
        let total_started_at = Instant::now();
        // --- Outbound runtime -> envelope -----------------------------------------
        let request_envelope_started_at = Instant::now();
        let request_envelope = match DanceEnvelopeAdapter::build_request_envelope(&context, request)
        {
            Ok(envelope) => envelope,
            Err(error) => return DanceResponse::from_error(error),
        };
        let request_envelope_millis = request_envelope_started_at.elapsed().as_millis();

        debug!("TrustChannel::initiate_dance() — prepared envelope request");

        // --- Transmit via backend --------------------------------------------
        let backend_started_at = Instant::now();
        let response_envelope = match self.backend.initiate_dance_envelope(request_envelope).await {
            Ok(envelope) => envelope,
            Err(error) => return DanceResponse::from_error(error),
        };
        let backend_millis = backend_started_at.elapsed().as_millis();

        // --- Inbound envelope -> runtime -------------------------------------
        let response_binding_started_at = Instant::now();
        let response =
            match DanceEnvelopeAdapter::bind_response_envelope(&context, response_envelope) {
                Ok(response) => response,
                Err(error) => return DanceResponse::from_error(error),
            };
        let response_binding_millis = response_binding_started_at.elapsed().as_millis();

        if is_load_holons {
            info!(
                "[PERF-688] trust_channel: request_envelope_ms={} backend_ms={} response_binding_ms={} total_ms={}",
                request_envelope_millis,
                backend_millis,
                response_binding_millis,
                total_started_at.elapsed().as_millis(),
            );
        }

        debug!("TrustChannel::initiate_dance() — got response: {:?}", response.summarize());
        response
    }
}
