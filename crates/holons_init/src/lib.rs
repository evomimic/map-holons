#[cfg(feature = "client")]
mod client_context_factory;

#[cfg(not(feature = "client"))]
mod guest_context_factory;

pub mod context_initialization;
mod holons_context_factory;

// Public exports
pub use context_initialization::init_context_from_session;
