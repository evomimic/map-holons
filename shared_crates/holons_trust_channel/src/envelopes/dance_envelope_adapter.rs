use core_types::HolonId;
use holons_boundary::DanceRequestWire;
use holons_boundary::HolonReferenceWire;
use holons_boundary::envelopes::{
    DanceEnvelopeRequest, DanceEnvelopeResponse, DanceRequestEnvelope, DanceResponseEnvelope,
};
use holons_boundary::session_state::SessionStateWire;
use holons_core::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceRequest, DanceResponse};
use std::sync::Arc;
use tracing::debug;

/// Adapter for translating runtime dance requests/responses into envelope transport types.
///
#[derive(Debug, Default)]
pub struct DanceEnvelopeAdapter;

/// Descriptor-registry capture boundary within request envelope construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryCaptureBoundary {
    Started,
    Finished,
}

impl DanceEnvelopeAdapter {
    /// Outbound: project runtime dance request + session into a transport envelope.
    pub fn build_request_envelope(
        context: &Arc<TransactionContext>,
        request: DanceRequest,
    ) -> Result<DanceRequestEnvelope, HolonError> {
        Self::build_request_envelope_with_observer(context, request, &mut |_| {})
    }

    /// Reports registry capture boundaries to the calling transport adapter.
    pub fn build_request_envelope_with_observer(
        context: &Arc<TransactionContext>,
        request: DanceRequest,
        observer: &mut impl FnMut(RegistryCaptureBoundary),
    ) -> Result<DanceRequestEnvelope, HolonError> {
        observer(RegistryCaptureBoundary::Started);
        let descriptor_registry = if request.dance_name.0 == "commit" {
            context
                .descriptor_registry()
                .map(|registry| holons_boundary::DescriptorRegistryWire::capture(&registry))
                .transpose()?
                .flatten()
        } else {
            None
        };
        observer(RegistryCaptureBoundary::Finished);
        let request_wire = DanceRequestWire::from(&request);
        let session = Some(Self::attach_session_state(context)?);
        Ok(DanceRequestEnvelope {
            request: DanceEnvelopeRequest::Legacy(request_wire),
            session,
            descriptor_registry,
        })
    }

    /// Inbound: hydrate context from envelope session state, then bind response wire to runtime.
    pub fn bind_response_envelope(
        context: &Arc<TransactionContext>,
        envelope: DanceResponseEnvelope,
    ) -> Result<DanceResponse, HolonError> {
        let DanceResponseEnvelope { response, session } = envelope;
        let session_state = session.ok_or_else(|| HolonError::InvalidWireFormat {
            wire_type: "InternalDanceResponseEnvelope".to_string(),
            reason: "Missing SessionStateWire".to_string(),
        })?;
        Self::hydrate_from_response(context, &session_state)?;
        match response {
            DanceEnvelopeResponse::Legacy(response) => response.bind(context),
            _ => Err(HolonError::InvalidParameter("Expected legacy Dance response".into())),
        }
    }

    /// Outbound: serializes staged and transient state into a wire payload.
    pub(crate) fn attach_session_state(
        context: &Arc<TransactionContext>,
    ) -> Result<SessionStateWire, HolonError> {
        SessionStateWire::from_context(context)
    }

    /// Inbound: restores staged and transient state from the wire payload.
    pub(crate) fn hydrate_from_response(
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
