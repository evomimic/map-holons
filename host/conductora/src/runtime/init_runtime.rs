use std::sync::Arc;

use client_shared_types::ReceptorType;
use holons_client::{ClientSession, Receptor, receptor_factory};
use tauri::{AppHandle, Manager};

use crate::runtime::RuntimeState;
use holons_client::{init_client_runtime}; 
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
    
    let recovery_receptor = get_recovery_receptor_from_factory(handle);

    let _client_session = ClientSession::new(space_manager.clone(), recovery_receptor,  None);

//TODO: use the client_session

    let session = Arc::new(RuntimeSession::new(space_manager));


    let runtime = Runtime::new(session);

    if let Some(state) = handle.try_state::<RuntimeState>() {
        match state.write() {
            Ok(mut guard) => {
                *guard = Some(runtime);
                tracing::info!("[RUNTIME] MAP Commands Runtime initialized.");
                return true;
            }
            Err(err) => {
                tracing::error!("[RUNTIME] Failed to acquire RuntimeState lock: {}", err);
                return false;
            }
        }
    }

    false
}

fn get_recovery_receptor_from_factory(handle: &AppHandle) -> Option<Arc<Receptor>> {
     handle.try_state::<receptor_factory::ReceptorFactory>()
    .and_then(|factory| {
        match factory.get_default_receptor_by_type(&ReceptorType::LocalRecovery) {
            Ok(receptor) => {
                tracing::info!("[RUNTIME] Local recovery receptor found.");      
                    return Some(receptor)
            }
            Err(err) => {
                tracing::warn!(
                    "[RUNTIME] Local recovery receptor unavailable ({}); recovery features disabled.",
                    err
                );
                return None
            }
        }
    });
    None
}
