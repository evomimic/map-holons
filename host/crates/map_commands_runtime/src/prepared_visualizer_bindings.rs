use std::collections::HashMap;
use std::sync::Mutex;

use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TxId;
use holons_core::HolonReference;
use map_commands_contract::{
    VisualizerOwner, VisualizerSelectionRequest, VisualizerUsageSelection,
};

/// Derived bindings for saved request inputs in one retained HX context.
/// Presentation objects, navigation state, and failed resolutions are never retained.
#[derive(Clone)]
pub(crate) struct PreparedVisualizerBinding {
    pub request: VisualizerSelectionRequest,
    pub selected: HolonReference,
    pub descriptor: HolonReference,
    pub usage: VisualizerUsageSelection,
    pub usage_type: HolonReference,
    pub alternatives_available: bool,
}

pub(crate) fn same_request(a: &VisualizerSelectionRequest, b: &VisualizerSelectionRequest) -> bool {
    a.subject == b.subject
        && a.requested_kind == b.requested_kind
        && a.slot == b.slot
        && a.theme == b.theme
        && match (&a.owner, &b.owner) {
            (VisualizerOwner::Visualizer(a), VisualizerOwner::Visualizer(b))
            | (VisualizerOwner::Dancer(a), VisualizerOwner::Dancer(b)) => a == b,
            _ => false,
        }
}

fn saved_request(request: &VisualizerSelectionRequest) -> bool {
    let owner = match &request.owner {
        VisualizerOwner::Visualizer(owner) | VisualizerOwner::Dancer(owner) => owner,
    };
    [&request.subject, &request.slot, &request.theme, owner]
        .iter()
        .all(|value| matches!(value, HolonReference::Smart(_)))
}

#[derive(Default)]
struct State {
    generation: u64,
    changing: usize,
    contexts: HashMap<TxId, Vec<PreparedVisualizerBinding>>,
}

/// Schema metadata follows the host's fixed space-service lifetime. Semantic
/// mutations, explicit choices, and explicit refresh retire the derived bindings.
/// Package/release refresh integrations must also invalidate before reuse.
#[derive(Default)]
pub(crate) struct PreparedVisualizerBindings(Mutex<State>);

impl PreparedVisualizerBindings {
    pub fn generation(&self) -> Result<u64, HolonError> {
        Ok(self.0.lock().map_err(lock_error)?.generation)
    }

    pub fn is_current(&self, generation: u64) -> Result<bool, HolonError> {
        let state = self.0.lock().map_err(lock_error)?;
        Ok(state.generation == generation && state.changing == 0)
    }

    pub fn get(
        &self,
        tx: TxId,
        request: &VisualizerSelectionRequest,
    ) -> Result<Option<PreparedVisualizerBinding>, HolonError> {
        if !saved_request(request) {
            return Ok(None);
        }
        let state = self.0.lock().map_err(lock_error)?;
        if state.changing != 0 {
            return Ok(None);
        }
        Ok(state
            .contexts
            .get(&tx)
            .and_then(|entries| entries.iter().find(|entry| same_request(&entry.request, request)))
            .cloned())
    }

    pub fn retain(
        &self,
        tx: TxId,
        generation: u64,
        binding: PreparedVisualizerBinding,
    ) -> Result<(), HolonError> {
        if !saved_request(&binding.request) {
            return Ok(());
        }
        let mut state = self.0.lock().map_err(lock_error)?;
        if state.generation != generation || state.changing != 0 {
            return Ok(());
        }
        let entries = state.contexts.entry(tx).or_default();
        entries.retain(|entry| !same_request(&entry.request, &binding.request));
        // Eviction only loses reuse; it never changes selection policy.
        if entries.len() == 256 {
            entries.remove(0);
        }
        entries.push(binding);
        Ok(())
    }

    pub fn begin_change(&self) -> Result<BindingChange<'_>, HolonError> {
        let mut state = self.0.lock().map_err(lock_error)?;
        state.changing += 1;
        state.generation += 1;
        state.contexts.clear();
        Ok(BindingChange(self))
    }

    pub fn invalidate(&self) -> Result<(), HolonError> {
        let mut state = self.0.lock().map_err(lock_error)?;
        state.generation += 1;
        state.contexts.clear();
        Ok(())
    }

    pub fn release(&self, tx: TxId) -> Result<(), HolonError> {
        let mut state = self.0.lock().map_err(lock_error)?;
        state.contexts.remove(&tx);
        // Do not allow an older in-flight resolution to resurrect disposed entries.
        state.generation += 1;
        Ok(())
    }
}

/// Keep reuse suspended for the complete semantic mutation, including failures.
pub(crate) struct BindingChange<'a>(&'a PreparedVisualizerBindings);
impl Drop for BindingChange<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0 .0.lock() {
            state.changing -= 1;
            state.generation += 1;
            state.contexts.clear();
        }
    }
}

fn lock_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::FailedToAcquireLock(error.to_string())
}
