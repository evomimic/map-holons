mod client_context;
mod client_shared_objects;
mod conductor_dance_caller;
mod dances_client;
pub mod deprecated_holochain_receptor;
pub mod holochain_conductor_client;
pub mod holochain_receptor;
mod host_signal;
mod storage_notification;

// Re-export key types and traits for external use
pub use deprecated_holochain_receptor::DeprecatedHolochainReceptor;
pub use holochain_conductor_client::HolochainConductorClient;
pub use holochain_receptor::HolochainReceptor;
// MAP-facing public API: identification-only notifications (no holon state)
pub use storage_notification::{MutationKind, StorageNotification};
// HostSignal, HolonsZomeSignal, decode_signal are adapter-internal — not re-exported
