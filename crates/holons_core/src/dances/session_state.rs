use crate::core_shared_objects::holon_pool::SerializableHolonPool;

use crate::HolonReference;
use serde::{Deserialize, Serialize};

/// SessionState provides a way to distinguish information associated with a specific request from
/// state info that is just being maintained via the ping pong process. This also should make it
/// easier to evolve to token-based state management approach where, say, the state token is
/// actually a reference into the ephemeral store.
/// The key_suffix_count is part of a workaround involving the hashing of key plus this suffix_count as a way to
/// create a unique temporary id for each Holon in the session specific Nursery.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct SessionState {
    transient_holons: SerializableHolonPool,
    staged_holons: SerializableHolonPool,
    local_holon_space: Option<HolonReference>,
    pub key_suffix_count: usize, // default beginning count is 0
}

impl SessionState {
    /// Creates a new session state with the provided staged and transient holons and local holon space.
    pub fn new(
        transient_holons: SerializableHolonPool,
        staged_holons: SerializableHolonPool,
        local_holon_space: Option<HolonReference>,
    ) -> Self {
        Self { transient_holons, staged_holons, local_holon_space, key_suffix_count: 1 }
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

    /// Retrieves the transient holon pool.
    pub fn get_transient_holons(&self) -> &SerializableHolonPool {
        &self.transient_holons
    }

    /// Retrieves a mutable reference to the transient holon pool.
    pub fn get_transient_holons_mut(&mut self) -> &mut SerializableHolonPool {
        &mut self.transient_holons
    }

    /// Sets a new staged holon pool.
    pub fn set_staged_holons(&mut self, staged_holons: SerializableHolonPool) {
        self.staged_holons = staged_holons;
    }

    /// Sets a new transient holon pool.
    pub fn set_transient_holons(&mut self, transient_holons: SerializableHolonPool) {
        self.transient_holons = transient_holons;
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
