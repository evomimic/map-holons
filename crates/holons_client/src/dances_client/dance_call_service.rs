//! Handles making dance calls while managing session state.
//!
use crate::dances_client::ConductorDanceCaller;

use holons_core::dances::{DanceRequest, DanceResponse, SessionState};
use holons_core::HolonsContextBehavior;
use tracing::{
    debug,
    // info,
    // warn,
};

/// A service that executes dance calls while managing session state.
///
/// This service is **generic over any conductor backend**, meaning it can be used with:
/// - A real Holochain conductor (`RealConductor`).
/// - A WASM-based JavaScript client (`WasmDanceClient`).
/// - A Sweetest mock conductor (`SweetestDanceClient`).
///
/// # Type Parameters
/// - `C`: A type implementing `DanceCaller`, which determines how the dance request is executed.
#[derive(Debug)]
pub struct DanceCallService<C: ConductorDanceCaller> {
    conductor: C,
}

impl<C: ConductorDanceCaller> DanceCallService<C> {
    /// Creates a new `DanceCallService` with a given conductor backend.
    ///
    /// # Arguments
    /// - `conductor`: A `DanceCaller` implementation that handles the actual request execution.
    pub fn new(conductor: C) -> Self {
        Self { conductor }
    }

    /// Executes a dance call while automatically managing session state.
    /// Asynchronous: loads session state into the request, awaits the conductor call,
    /// restores session state from the response, and returns the response.
    pub async fn dance_call(
        &self,
        context: &dyn HolonsContextBehavior,
        mut request: DanceRequest,
    ) -> DanceResponse {
        // info!("entered dance call with context: {context} for request: {request:?}");

        // 1. Load session state into the request
        let mut session_state = SessionState::default();
        self.load_session_state(context, &mut session_state);
        request.state = Some(session_state);

        debug!("\n\n DANCE_CALL_REQUEST :: {:#?}", request.summarize());

        // 2. Execute the dance call
        let response = self.conductor.conductor_dance_call(request).await;
        // 3. Ensure the response includes a valid session state
        assert!(
            response.state.is_some(),
            "DanceResponse is missing session state, which should never happen"
        );

        // 4. Restore session state from the response
        let response_session_state = response.state.as_ref().unwrap();
        self.load_nursery(context, response_session_state);
        self.load_transient_manager(context, response_session_state);

        // 5. Update space manager's local_holon_space without moving response
        let space_manager = context.get_space_manager();
        space_manager.set_space_holon(response_session_state.get_local_holon_space().unwrap());

        response
    }

    /// Loads the current session state from the managers into the given `SessionState` instance.
    ///
    /// This function retrieves staged holons from the HolonSpaceManager and injects them into
    /// the provided `session_state`, ensuring that the outgoing `DanceRequest` includes
    /// the latest state from the local context.
    ///
    /// # Arguments
    /// * `context` - A reference to the `HolonsContextBehavior`, which provides access to the space manager.
    /// * `session_state` - A mutable reference to the `SessionState` that will be updated with staged holons.
    ///
    /// This function is called automatically within `dance_call` and should not be used directly.
    fn load_session_state(
        &self,
        context: &dyn HolonsContextBehavior,
        session_state: &mut SessionState,
    ) {
        let space_manager = context.get_space_manager();
        let staged_holons = space_manager.export_staged_holons();
        let transient_holons = space_manager.export_transient_holons();
        session_state.set_staged_holons(staged_holons);
        session_state.set_transient_holons(transient_holons);
        session_state.set_local_holon_space(space_manager.get_space_holon());
    }

    /// Restores the nursery from the given `SessionState`, updating the local HolonSpace.
    ///
    /// This function takes the staged holons stored in the `session_state` (as received in a `DanceResponse`)
    /// and imports them back into the HolonSpaceManager, ensuring that the local environment remains
    /// synchronized with the session state maintained by the client and guest.
    ///
    /// # Arguments
    /// * `context` - A reference to the `HolonsContextBehavior`, used to access the space manager.
    /// * `session_state` - A reference to the `SessionState` from which staged holons will be restored.
    ///
    /// This function is automatically invoked within `dance_call` after receiving a response and should not
    /// be used directly.
    fn load_nursery(&self, context: &dyn HolonsContextBehavior, session_state: &SessionState) {
        let space_manager = context.get_space_manager();
        let staged_holons = session_state.get_staged_holons().clone();
        space_manager.import_staged_holons(staged_holons);
    }

    /// Restores the TransientHolonManager from the given `SessionState`, updating the local HolonSpace.
    ///
    /// Takes the transients holons stored in the `session_state` (as received in a `DanceResponse`)
    /// and imports them back into the HolonSpaceManager, ensuring that the local environment remains
    /// synchronized with the session state maintained by the client and guest.
    ///
    /// # Arguments
    /// * `context` - A reference to the `HolonsContextBehavior`, used to access the space manager.
    /// * `session_state` - A reference to the `SessionState` from which transient holons will be restored.
    ///
    /// This function is automatically invoked within `dance_call` after receiving a response and should not
    /// be used directly.
    fn load_transient_manager(
        &self,
        context: &dyn HolonsContextBehavior,
        session_state: &SessionState,
    ) {
        let space_manager = context.get_space_manager();
        let transient_holons = session_state.get_transient_holons().clone();
        space_manager.import_transient_holons(transient_holons);
    }
}
