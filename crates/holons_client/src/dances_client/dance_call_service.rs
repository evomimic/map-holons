//! Handles making dance calls while managing session state.
//!
use crate::dances_client::ConductorDanceCaller;

use holons_core::dances::{DanceRequest, DanceResponse, SessionState};
use holons_core::HolonsContextBehavior;

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
    ///
    /// - Loads session state into the request.
    /// - Sends the request via the provided `DanceCaller` implementation.
    /// - Restores session state from the response.
    ///
    /// This function is **synchronous** because all conductor calls are synchronous.
    pub fn dance_call(
        &self,
        context: &dyn HolonsContextBehavior,
        mut request: DanceRequest,
    ) -> DanceResponse {
        // 1. Load session state into the request if it’s missing
        if request.state.is_none() {
            let mut session_state = SessionState::default();
            self.load_session_state(context, &mut session_state);
            request.state = Some(session_state);
        }

        // 2. Execute the dance call
        let response = self.conductor.conductor_dance_call(request);

        // 3. Ensure the response includes a valid session state
        assert!(
            response.state.is_some(),
            "DanceResponse is missing session state, which should never happen"
        );

        // 4. Restore session state from the response
        self.load_nursery(context, response.state.as_ref().unwrap());

        response
    }

    /// Loads the current session state from the nursery into the given `SessionState` instance.
    ///
    /// This function retrieves staged holons from the HolonSpaceManager and injects them into
    /// the provided `session_state`, ensuring that the outgoing `DanceRequest` includes
    /// the latest state from the local context.
    ///
    /// # Arguments
    ///
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
        session_state.set_staged_holons(staged_holons);
    }

    /// Restores the nursery from the given `SessionState`, updating the local HolonSpace.
    ///
    /// This function takes the staged holons stored in the `session_state` (as received in a `DanceResponse`)
    /// and imports them back into the HolonSpaceManager, ensuring that the local environment remains
    /// synchronized with the session state maintained by the client and guest.
    ///
    /// # Arguments
    ///
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
}
