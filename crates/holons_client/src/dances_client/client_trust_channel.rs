use std::sync::Arc;

use derive_new::new;
use holons_core::dances::ConductorDanceCaller;

/// Handles outbound dance calls from the client to a guest via the conductor.
#[derive(new, Debug, Clone)]
pub struct ClientDanceCaller {
    conductor: Arc<dyn ConductorDanceCaller>, // client-side transport type
}

// #[async_trait(?Send)]
// impl ConductorDanceCaller for ClientDanceCaller {
//     async fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse {
//         // Replace this with your actual conductor API call
//         self.conductor.call("holons", "dance", request).await
//     }
// }
