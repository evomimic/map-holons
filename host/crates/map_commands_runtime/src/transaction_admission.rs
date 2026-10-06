use std::sync::{Arc, Mutex};

use core_types::HolonError;
use holons_core::HolonReference;

/// Host command admission and one-shot loader submission share one atomic boundary.
/// Retained references are cleared on explicit disposal; terminal outcomes never reset submission.
#[derive(Debug, Default)]
pub(crate) struct TransactionAdmission {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    active: usize,
    disposed: bool,
    submitted: bool,
    preparing: bool,
    prepared: Option<HolonReference>,
}

pub(crate) struct CommandLease {
    admission: Arc<TransactionAdmission>,
    mutating: bool,
}

impl Drop for CommandLease {
    fn drop(&mut self) {
        if let Ok(mut state) = self.admission.state.lock() {
            state.active -= 1;
            if self.mutating {
                state.preparing = false;
            }
        }
    }
}

impl TransactionAdmission {
    pub fn enter(self: &Arc<Self>, mutating: bool) -> Result<CommandLease, HolonError> {
        let mut state =
            self.state.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        if state.disposed || (mutating && (state.submitted || state.preparing)) {
            return Err(HolonError::InvalidState(
                "Transaction is disposed or its load has already been submitted".into(),
            ));
        }
        state.active += 1;
        Ok(CommandLease { admission: Arc::clone(self), mutating })
    }

    pub fn begin_preparation(&self) -> Result<(), HolonError> {
        let mut state =
            self.state.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        if state.disposed || state.submitted || state.active != 1 {
            return Err(HolonError::InvalidState(
                "Preparation requires an idle, unsubmitted transaction".into(),
            ));
        }
        state.preparing = true;
        state.prepared = None;
        Ok(())
    }

    pub fn prepared(&self, request: HolonReference) -> Result<(), HolonError> {
        let mut state =
            self.state.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        if state.submitted || state.disposed {
            return Err(HolonError::InvalidState("Load interaction is no longer preparing".into()));
        }
        state.prepared = Some(request);
        Ok(())
    }

    pub fn submit(&self, request: &HolonReference) -> Result<(), HolonError> {
        let mut state =
            self.state.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        if state.disposed
            || state.submitted
            || state.prepared.as_ref() != Some(request)
            || state.active != 1
        {
            return Err(HolonError::InvalidState(
                "Load requires its prepared request and exclusive, first submission".into(),
            ));
        }
        state.submitted = true;
        Ok(())
    }

    pub fn dispose(&self) -> Result<(), HolonError> {
        let mut state =
            self.state.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        if state.active != 0 {
            return Err(HolonError::InvalidState(
                "Cannot dispose a transaction while a command is executing".into(),
            ));
        }
        state.disposed = true;
        state.prepared = None;
        Ok(())
    }
}
