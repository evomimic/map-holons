use core_types::HolonId;
use holons_boundary::envelopes::{InternalDanceRequestEnvelope, InternalDanceResponseEnvelope};
use holons_boundary::session_state::SerializableHolonPool;
use holons_boundary::DanceRequestWire;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_boundary::session_state::SessionStateWire;
use holons_core::dances::{DanceRequest, DanceResponse};
use holons_core::{HolonError, HolonsContextBehavior};
use holons_boundary::HolonReferenceWire;
use std::sync::Arc;
use tracing::debug;

/// Adapter for translating runtime dance requests/responses into envelope transport types.
///
#[derive(Debug, Default)]
pub struct DanceEnvelopeAdapter;

impl DanceEnvelopeAdapter {
    /// Outbound: project runtime dance request + session into a transport envelope.
    pub fn build_request_envelope(
        context: &Arc<TransactionContext>,
        request: DanceRequest,
    ) -> Result<InternalDanceRequestEnvelope, HolonError> {
        let request_wire = DanceRequestWire::from(&request);
        let session = Some(Self::attach_session_state(context)?);
        Ok(InternalDanceRequestEnvelope { request: request_wire, session })
    }

    /// Inbound: hydrate context from envelope session state, then bind response wire to runtime.
    pub fn bind_response_envelope(
        context: &Arc<TransactionContext>,
        envelope: InternalDanceResponseEnvelope,
    ) -> Result<DanceResponse, HolonError> {
        let InternalDanceResponseEnvelope { response, session } = envelope;
        let session_state = session.ok_or_else(|| {
            HolonError::InvalidWireFormat {
                wire_type: "InternalDanceResponseEnvelope".to_string(),
                reason: "Missing SessionStateWire".to_string(),
            }
        })?;
        Self::hydrate_from_response(context, &session_state)?;
        response.bind(context)
    }

    /// Outbound: serializes staged and transient state into a wire payload.
    fn attach_session_state(context: &Arc<TransactionContext>) -> Result<SessionStateWire, HolonError> {
        let mut session_state = SessionStateWire::default();

        let staged_pool = context.export_staged_holons()?;
        let transient_pool = context.export_transient_holons()?;

        session_state.set_staged_holons(SerializableHolonPool::from(&staged_pool));
        session_state.set_transient_holons(SerializableHolonPool::from(&transient_pool));
        session_state.set_local_holon_space(context.get_space_holon()?);
        session_state.set_tx_id(context.tx_id());

        Ok(session_state)
    }

    /// Inbound: restores staged and transient state from the wire payload.
    fn hydrate_from_response(
        context: &Arc<TransactionContext>,
        state: &SessionStateWire,
    ) -> Result<(), HolonError> {
        let response_tx_id = state
            .get_tx_id()
            .ok_or_else(|| HolonError::InvalidParameter("SessionState missing tx_id".into()))?;
        if response_tx_id != context.tx_id() {
            return Err(HolonError::CrossTransactionReference {
                reference_kind: "SessionState".to_string(),
                reference_id: format!("TxId={}", response_tx_id.value()),
                reference_tx: response_tx_id.value(),
                context_tx: context.tx_id().value(),
            });
        }

        let bound_staged_holons = state.get_staged_holons().clone().bind(context)?;
        let bound_transient_holons = state.get_transient_holons().clone().bind(context)?;

        context.import_staged_holons(bound_staged_holons)?;
        context.import_transient_holons(bound_transient_holons)?;

        // Space holon anchor is stored as a wire reference for now; extract HolonId without context_binding.
        if let Some(space_ref_wire) = state.get_local_space_holon_wire() {
            let space_holon_id = space_holon_id_from_wire_reference(&space_ref_wire)?;
            context.set_space_holon_id(space_holon_id)?;
        }

        debug!("DanceEnvelopeAdapter::hydrate_from_response() — {}", state.summarize());
        Ok(())
    }
}

// TEMPORARY: remove once SessionState stores HolonId directly
/// Extracts the persisted holon id from a wire reference suitable for anchoring the space holon.
///
/// The space holon must always be persisted, so only SmartReferenceWire is accepted.
fn space_holon_id_from_wire_reference(
    reference_wire: &HolonReferenceWire,
) -> Result<HolonId, HolonError> {
    match reference_wire {
        HolonReferenceWire::Smart(smart_wire) => Ok(smart_wire.holon_id().clone()),
        other => Err(HolonError::InvalidHolonReference(format!(
            "Space holon must be a SmartReferenceWire; got {:?}",
            other
        ))),
    }
}
