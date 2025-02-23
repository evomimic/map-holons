use crate::shared_test::{setup_conductor, MockConductorConfig};
use futures::TryFutureExt;
use holochain::prelude::event::KitsuneP2pEventSender;
use holochain::sweettest::{SweetCell, SweetConductor};
use holons_core::dances::{DanceRequest, DanceResponse, SessionState};
use holons_core::HolonsContextBehavior;

#[derive(Debug)]
pub struct DanceCallService {
    conductor_config: MockConductorConfig,
}
impl DanceCallService {
    pub async fn init() -> Self {
        let conductor_config = setup_conductor().await;
        DanceCallService { conductor_config }
    }

    pub async fn dance_call(
        &self,
        context: &dyn HolonsContextBehavior,
        mut request: DanceRequest,
    ) -> DanceResponse {
        // 1. Load session state into the request
        self.load_session_state(context, &mut request.state);

        // 2. Make the dance call
        let response: DanceResponse = self
            .conductor_config
            .conductor
            .call(&self.conductor_config.cell.zome("dances"), "dance", request)
            .await;

        // 3. Update the nursery with the session state from the response
        self.load_nursery(context, &response.state);

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
        let space_manager = context.get_space_manager(); // Arc<dyn HolonSpaceBehavior>
        let staged_holons = space_manager.export_staged_holons(); // No borrowing needed

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
        let space_manager = context.get_space_manager(); // Arc<dyn HolonSpaceBehavior>
        let staged_holons = session_state.get_staged_holons().clone(); // Clone for ownership

        space_manager.import_staged_holons(staged_holons); // Delegate to HolonSpaceManager
    }
}
