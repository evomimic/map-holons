use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::RwLock;

use core_types::HolonError;

use crate::holon_space::SpaceInfo;

/// Minimal interface that any runtime-capable storage receptor implements.
///
/// The Tauri state slot (`ActiveStorageReceptor`) holds `Arc<dyn StorageReceptor>`,
/// making it independent of the concrete receptor type (Holochain, Local, etc.)
/// that was wired up at config time.
///
/// Signal-subscription methods are on the concrete type only. The MAP-facing
/// public API is `subscribe_action_events() -> Receiver<ActionEvent>`;
/// `subscribe_decoded()` is adapter-internal. Callers should downcast via
/// `Arc::downcast` to access these on `HolochainReceptor`.
pub trait StorageReceptor: Send + Sync {
    fn receptor_id(&self) -> &str;

    fn get_space_info(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<SpaceInfo, HolonError>> + Send + '_>>;
}

/// Tauri managed-state slot for whichever storage receptor was configured at startup.
///
/// Written by the provider setup path (e.g. `HolochainSetup::setup`).
/// Read by status commands, space queries, and any code that needs receptor-level access
/// without going through `RuntimeSession` / `dispatch_map_command`.
pub type ActiveStorageReceptor = RwLock<Option<Arc<dyn StorageReceptor>>>;
