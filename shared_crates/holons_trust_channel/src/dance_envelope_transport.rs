use async_trait::async_trait;
use core_types::HolonError;
use holons_boundary::envelopes::{InternalDanceRequestEnvelope, InternalDanceResponseEnvelope};
use std::fmt::Debug;

/// Canonical transport interface for outbound dance envelopes.
#[async_trait]
pub trait DanceEnvelopeTransport: Send + Sync + Debug {
    async fn initiate_dance_envelope(
        &self,
        envelope: InternalDanceRequestEnvelope,
    ) -> Result<InternalDanceResponseEnvelope, HolonError>;
}
