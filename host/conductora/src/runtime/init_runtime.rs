use std::sync::Arc;

use client_shared_types::ReceptorType;
use holons_client::{init_client_runtime, receptor_factory, Receptor};
use map_commands_runtime::{Runtime, RuntimeSession};
use tauri::{AppHandle, Manager};

use crate::runtime::RuntimeState;

/// Stored by providers (e.g. Holochain) and consumed by runtime init.
pub type RuntimeInitiatorState =
    std::sync::RwLock<Option<Arc<dyn holons_core::dances::DanceInitiator>>>;

/// Initialize the MAP Commands runtime from the initiator stored in app state.
///
/// Target architecture:
/// - build the runtime HolonSpaceManager
/// - resolve optional recovery receptor
/// - construct a recovery-aware RuntimeSession
/// - restore any orphaned/open sessions before publishing Runtime
pub fn init_from_state(handle: &AppHandle) -> bool {
    let initiator =
        handle.try_state::<RuntimeInitiatorState>().and_then(|state| state.read().ok()?.clone());

    let Some(initiator) = initiator else {
        tracing::warn!(
            "[RUNTIME] No runtime initiator available - MAP Commands Runtime will not be initialized."
        );
        return false;
    };

    let space_manager = init_client_runtime(Some(initiator));
    let recovery_receptor = get_recovery_receptor_from_factory(handle);

    let session = Arc::new(RuntimeSession::new(
        Arc::clone(&space_manager),
        recovery_receptor.clone(),
    ));

    if recovery_receptor.is_some() {
        match session.restore_open_sessions() {
            Ok(restored) => {
                tracing::info!(
                    "[RUNTIME] Runtime session initialized. Restored {} recovery session(s).",
                    restored
                );
            }
            Err(err) => {
                tracing::error!(
                    "[RUNTIME] Failed to restore recovery sessions during startup: {}",
                    err
                );
                return false;
            }
        }
    }

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

    tracing::error!("[RUNTIME] RuntimeState missing; runtime could not be stored.");
    false
}

fn get_recovery_receptor_from_factory(handle: &AppHandle) -> Option<Arc<Receptor>> {
    let factory = handle.try_state::<receptor_factory::ReceptorFactory>()?;

    match factory.get_default_receptor_by_type(&ReceptorType::LocalRecovery) {
        Ok(receptor) => {
            tracing::info!("[RUNTIME] Local recovery receptor found.");
            Some(receptor)
        }
        Err(err) => {
            tracing::warn!(
                "[RUNTIME] Local recovery receptor unavailable ({}); recovery features disabled.",
                err
            );
            None
        }
    }
}
