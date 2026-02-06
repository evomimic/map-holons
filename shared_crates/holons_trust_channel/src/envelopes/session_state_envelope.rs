use core_types::HolonId;
use holons_boundary::session_state::SerializableHolonPool;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceRequest, DanceResponse, SessionState};
use holons_core::{HolonError, HolonReferenceWire, HolonsContextBehavior};
use std::sync::Arc;
use tracing::debug;

/// The SessionStateEnvelope layer manages attaching and restoring SessionState
/// during outbound and inbound capsule flow.
///
/// It wraps the existing `holons_core::dances::SessionState` model and provides
/// runtime logic for interacting with the SpaceManager context.
#[derive(Debug, Default)]
pub struct SessionStateEnvelope;

impl SessionStateEnvelope {
    /// Outbound: serializes staged and transient state into the request.
    ///
    /// Inject the current session_state state into a DanceRequest before sending.
    pub fn attach_to_request(
        context: &Arc<TransactionContext>,
        request: &mut DanceRequest,
    ) -> Result<(), HolonError> {
        let mut session_state = SessionState::default();

        let staged_pool = context.export_staged_holons()?;
        let transient_pool = context.export_transient_holons()?;

        session_state.set_staged_holons(SerializableHolonPool::from(&staged_pool));
        session_state.set_transient_holons(SerializableHolonPool::from(&transient_pool));
        session_state.set_local_holon_space(context.get_space_holon()?);
        session_state.set_tx_id(context.tx_id());

        request.state = Some(session_state);
        debug!("SessionStateEnvelope::attach_to_request() — {}", request.summarize());
        Ok(())
    }

    /// Inbound: restores staged and transient state from the response.
    ///
    /// Hydrate the local environment (nursery, transient manager, and local holon)
    /// from the SessionState contained in a DanceResponse.
    pub fn hydrate_from_response(
        context: &Arc<TransactionContext>,
        response: &DanceResponse,
    ) -> Result<(), HolonError> {
        let Some(state) = &response.state else {
            return Err(HolonError::InvalidParameter("DanceResponse missing SessionState".into()));
        };
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

        debug!("SessionStateEnvelope::hydrate_from_response() — {}", state.summarize());
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
