use hdk::prelude::*;

use hdi::hdk_entry_helper;
use holons_core::core_shared_objects::holon_pool::SerializableHolonPool;
use holons_core::reference_layer::HolonReference;

/// SessionState provides a way to distinguish information associated with a specific request from
/// state info that is just being maintained via the ping pong process. This also should make it
/// easier to evolve to token-based state management approach where, say, the state token is
/// actually a reference into the ephemeral store.
#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct SessionState {
    staged_holons: SerializableHolonPool,
    local_holon_space: Option<HolonReference>,
}

impl SessionState {
    /// Creates an empty session state.
    pub fn empty() -> Self {
        Self { staged_holons: SerializableHolonPool::default(), local_holon_space: None }
    }

    /// Creates a new session state with the provided staged holons and local holon space.
    pub fn new(
        staged_holons: SerializableHolonPool,
        local_holon_space: Option<HolonReference>,
    ) -> Self {
        Self { staged_holons, local_holon_space }
    }

    pub fn get_local_holon_space(&self) -> Option<HolonReference> {
        self.local_holon_space.clone()
    }
    /// Sets a new local holon space reference.
    pub fn set_local_holon_space(&mut self, local_holon_space: Option<HolonReference>) {
        self.local_holon_space = local_holon_space;
    }

    /// Retrieves the staged holon pool.
    pub fn get_staged_holons(&self) -> &SerializableHolonPool {
        &self.staged_holons
    }

    /// Retrieves a mutable reference to the staged holon pool.
    pub fn get_staged_holons_mut(&mut self) -> &mut SerializableHolonPool {
        &mut self.staged_holons
    }

    /// Sets a new staged holon pool.
    pub fn set_staged_holons(&mut self, staged_holons: SerializableHolonPool) {
        self.staged_holons = staged_holons;
    }

    /// Summarizes the session state.
    pub fn summarize(&self) -> String {
        format!(
            "\n   local_holon_space: {:?}, \n  staged holons: {} }}",
            self.local_holon_space,
            self.staged_holons.holons.len(),
        )
    }
}
