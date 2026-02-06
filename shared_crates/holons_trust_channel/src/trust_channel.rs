use async_trait::async_trait;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceRequest, DanceResponse};
use std::sync::Arc;
use tracing::debug;

use crate::envelopes::session_state_envelope::SessionStateEnvelope;
use holons_core::dances::dance_initiator::DanceInitiator;

/// The TrustChannel coordinates envelope flow for outbound and inbound Dances.
///
/// It wraps an inner [`DanceInitiator`] backend and applies envelope logic,
/// including session_state-state encapsulation, before and after the core invocation.
#[derive(Debug, Clone)]
pub struct TrustChannel {
    backend: std::sync::Arc<dyn DanceInitiator + Send + Sync>,
}

impl TrustChannel {
    /// Constructs a new TrustChannel around a backend initiator.
    pub fn new(backend: std::sync::Arc<dyn DanceInitiator + Send + Sync>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl DanceInitiator for TrustChannel {
    async fn initiate_dance(
        &self,
        context: Arc<TransactionContext>,
        mut request: DanceRequest,
    ) -> DanceResponse {
        // --- Outbound session_state state encapsulation -----------------------------
        if let Err(err) = SessionStateEnvelope::attach_to_request(&context, &mut request) {
            return DanceResponse::from_error(err);
        }

        debug!("TrustChannel::initiate_dance() — prepared request: {:?}", request.summarize());

        // --- Transmit via backend --------------------------------------------
        let context_for_backend = Arc::clone(&context);

        let mut response = self.backend.initiate_dance(context_for_backend, request).await;

        // --- Inbound session_state state hydration ---------------------------------
        if let Err(err) = SessionStateEnvelope::hydrate_from_response(&context, &response) {
            // Instead of discarding the response, annotate it with local error context.
            response.annotate_error(err);
        }

        debug!("TrustChannel::initiate_dance() — got response: {:?}", response.summarize());

        response
    }
}
