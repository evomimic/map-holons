use holons_core::dances::{DanceRequest, DanceResponse, SessionState};
use holons_core::{HolonError, HolonsContextBehavior};
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
    /// Inject the current session state into a DanceRequest before sending.
    pub fn attach_to_request(
        context: &dyn HolonsContextBehavior,
        request: &mut DanceRequest,
    ) -> Result<(), HolonError> {
        let mut session_state = SessionState::default();

        session_state.set_staged_holons(context.export_staged_holons()?);
        session_state.set_transient_holons(context.export_transient_holons()?);
        session_state.set_local_holon_space(context.get_space_holon()?);

        request.state = Some(session_state);
        debug!("SessionStateEnvelope::attach_to_request() — {}", request.summarize());
        Ok(())
    }

    /// Inbound: restores staged and transient state from the response.
    ///
    /// Hydrate the local environment (nursery, transient manager, and local holon)
    /// from the SessionState contained in a DanceResponse.
    pub fn hydrate_from_response(
        context: &dyn HolonsContextBehavior,
        response: &DanceResponse,
    ) -> Result<(), HolonError> {
        let Some(state) = &response.state else {
            return Err(HolonError::InvalidParameter("DanceResponse missing SessionState".into()));
        };
        let space_manager = context.get_space_manager();
        context.import_staged_holons(state.get_staged_holons().clone());
        context.import_transient_holons(state.get_transient_holons().clone());

        if let Some(space_ref) = state.get_local_holon_space() {
            space_manager.set_space_holon(space_ref.clone())?;
        }

        debug!("SessionStateEnvelope::hydrate_from_response() — {}", state.summarize());
        Ok(())
    }
}
