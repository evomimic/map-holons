//! Process exclusion for the supported native runtime and offline credential cutover.
use std::{
    fs::{File, OpenOptions},
    path::Path,
    sync::Mutex,
};

/// Advisory exclusion shared by current native runtimes and offline tooling.
/// Older binaries and independently opened database handles must be stopped separately.
pub(crate) struct InstallationLease(File);

impl InstallationLease {
    pub(crate) fn acquire(root: &Path) -> crate::Result<Self> {
        let metadata = root.symlink_metadata()?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(crate::Error::NativeBoundary(
                "Installation root is not a real directory".into(),
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(root.join(".native-runtime.lock"))?;
        if !file.metadata()?.is_file() {
            return Err(crate::Error::NativeBoundary(
                "Installation lock is not a regular file".into(),
            ));
        }
        file.try_lock().map_err(|_| {
            crate::Error::NativeBoundary(
                "Installation is in use; stop its runtime before offline maintenance".into(),
            )
        })?;
        Ok(Self(file))
    }
}

/// Keep exclusion if the runtime is abandoned without confirmed shutdown.
/// Releasing on an ordinary Drop could leave conductor tasks using the store.
pub(crate) struct RuntimeInstallationLease(Mutex<Option<InstallationLease>>);
impl RuntimeInstallationLease {
    pub(crate) fn new(lease: InstallationLease) -> Self {
        Self(Mutex::new(Some(lease)))
    }
    pub(crate) fn release_after_shutdown(&self) {
        self.0.lock().unwrap().take();
    }
}
impl Drop for RuntimeInstallationLease {
    fn drop(&mut self) {
        if let Some(lease) = self.0.get_mut().unwrap().take() {
            // Fail closed until process exit when the caller did not shut down.
            std::mem::forget(lease.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn probe(root: &Path, busy: bool) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "installation_lock::tests::child_process_lock_probe"])
            .env("MAP_TEST_LOCK_ROOT", root)
            .env("MAP_TEST_LOCK_BUSY", if busy { "yes" } else { "no" })
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child lock probe failed: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    #[test]
    fn child_process_lock_probe() {
        if let Some(root) = std::env::var_os("MAP_TEST_LOCK_ROOT") {
            let result = InstallationLease::acquire(Path::new(&root));
            assert_eq!(result.is_err(), std::env::var("MAP_TEST_LOCK_BUSY").unwrap() == "yes");
        }
    }

    #[test]
    fn exclusion_releases_only_after_confirmed_shutdown() {
        let root = std::env::temp_dir().join(format!("map-lease-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let lease = RuntimeInstallationLease::new(InstallationLease::acquire(&root).unwrap());
        assert!(InstallationLease::acquire(&root).is_err());
        probe(&root, true);
        lease.release_after_shutdown();
        probe(&root, false);
        drop(InstallationLease::acquire(&root).unwrap());
        let abandoned = RuntimeInstallationLease::new(InstallationLease::acquire(&root).unwrap());
        drop(abandoned);
        probe(&root, true);
        std::fs::remove_dir_all(root).unwrap();
    }
}
