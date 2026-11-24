//! Serialize explicit entry, restart and environment changes.
#[derive(Default)]
pub struct NavigatorLaunchLock {
    lock: tokio::sync::Mutex<()>,
    restarting: std::sync::atomic::AtomicBool,
}

impl NavigatorLaunchLock {
    pub fn try_acquire(&self) -> Result<tokio::sync::MutexGuard<'_, ()>, String> {
        use std::sync::atomic::Ordering;
        let guard =
            self.lock.try_lock().map_err(|_| "A launch or restart is already in progress")?;
        if self.restarting.load(Ordering::SeqCst) {
            return Err("MAP is restarting; environment changes are unavailable".into());
        }
        Ok(guard)
    }

    pub fn mark_restarting(&self) {
        self.restarting.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_and_restart_cannot_overlap() {
        let lock = NavigatorLaunchLock::default();
        let launch = lock.try_acquire().unwrap();
        assert!(lock.try_acquire().is_err());
        drop(launch);
        assert!(lock.try_acquire().is_ok());
        lock.mark_restarting();
        assert!(lock.try_acquire().is_err());
    }
}
