use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::runtime::RuntimeState;
use holons_client::init_client_runtime;
use map_commands::dispatch::{Runtime, RuntimeSession};

/// Stored by providers (e.g. Holochain) and consumed by runtime init.
pub type RuntimeInitiatorState = std::sync::RwLock<Option<Arc<dyn holons_core::dances::DanceInitiator>>>;

/// Initialize the MAP Commands runtime from the initiator stored in app state.
pub fn init_from_state(handle: &AppHandle) -> bool {
    let initiator = handle
        .try_state::<RuntimeInitiatorState>()
        .and_then(|state| state.read().ok()?.clone());

    let Some(initiator) = initiator else {
        tracing::warn!(
            "[RUNTIME] No runtime initiator available — MAP Commands Runtime will not be initialized."
        );
        return false;
    };

    let space_manager = init_client_runtime(Some(initiator));
    let session = Arc::new(RuntimeSession::new(space_manager));
    let runtime = Runtime::new(session);

    if let Some(state) = handle.try_state::<RuntimeState>() {
        let mut guard = state.write().expect("RuntimeState lock poisoned");
        *guard = Some(runtime);
        tracing::info!("[RUNTIME] MAP Commands Runtime initialized.");
        return true;
    }

    false
}
