//! Space-managed saved-state lifetime, independent of public transaction lifetime.
use core_types::HolonError;
use std::sync::Mutex;

/// One shared retirement token for registered products and in-flight derived preparation.
/// Detached Commit copies deliberately carry no host Session token.
#[derive(Debug, Default)]
pub struct SpaceSession {
    retired: Mutex<bool>,
}

impl SpaceSession {
    /// Reports the specific condition that ordinary descriptor wrappers may recover from.
    pub fn check_active(&self) -> Result<(), HolonError> {
        self.publish(|| Ok(()))
    }

    /// Atomically admits a short cache publication against retirement. The closure
    /// must not perform transport, reenter Session acquisition, or acquire another token.
    pub fn publish<T>(
        &self,
        publish: impl FnOnce() -> Result<T, HolonError>,
    ) -> Result<T, HolonError> {
        let retired =
            self.retired.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        if *retired {
            return Err(HolonError::ObsoleteSession);
        }
        publish()
    }

    pub(super) fn retire(&self) -> Result<(), HolonError> {
        *self.retired.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))? = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn retirement_is_shared_and_excludes_late_publication() {
        let session = Arc::new(SpaceSession::default());
        let retained = session.clone();
        session.publish(|| Ok(())).unwrap();
        session.retire().unwrap();
        assert_eq!(retained.check_active(), Err(HolonError::ObsoleteSession));
        assert_eq!(
            retained.publish(|| panic!("retired publication must not run")),
            Err::<(), _>(HolonError::ObsoleteSession)
        );
        assert!(SpaceSession::default().check_active().is_ok());
    }
}
