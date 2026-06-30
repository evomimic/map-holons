use std::sync::Arc;

use holons_client::{init_client_runtime, LocalRecoveryReceptor}; //, receptor_factory};
use map_commands_runtime::{Runtime, RuntimeSession};
use tauri::{AppHandle, Manager};

use crate::runtime::RuntimeState;

/// Stored by providers (e.g. Holochain) and consumed by runtime init.
pub type RuntimeInitiatorState =
    std::sync::RwLock<Option<Arc<dyn holons_core::dances::DanceInitiator>>>;

/// Typed state slot for the local recovery receptor.
/// Written by local/setup.rs, read by init_from_state.
pub type RecoveryReceptorState = std::sync::RwLock<Option<Arc<LocalRecoveryReceptor>>>;

/// Typed state slot for the Holochain conductor client.
/// Written by holochain/setup.rs, read by all_spaces and status commands.
//pub type HolochainReceptorState =
//   std::sync::RwLock<Option<Arc<HolochainConductorClient>>>;

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

    let recovery_receptor =
        handle.try_state::<RecoveryReceptorState>().and_then(|state| state.read().ok()?.clone());

    let session =
        Arc::new(RuntimeSession::new(Arc::clone(&space_manager), recovery_receptor.clone()));

    if recovery_receptor.is_some() {
        if crate::env::hc_dev_mode_enabled() {
            tracing::info!(
                "[RUNTIME] Startup session recovery suppressed: HC_DEV_MODE is enabled."
            );
        } else {
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
