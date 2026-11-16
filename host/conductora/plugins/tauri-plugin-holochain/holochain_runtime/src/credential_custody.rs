//! Native credential custody. This unlocks technical keys; it grants no MAP authority.
use lair_keystore_api::{dependencies::sodoken, types::SharedLockedArray};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;

#[cfg(target_os = "macos")]
mod macos;
mod rewrap;
pub use rewrap::{prepare_passphrase_transition, rewrap_lair_config, PreparedCredentialWrappers};

fn denied(message: impl Into<String>) -> crate::Error {
    crate::Error::NativeBoundary(message.into())
}

/// Only the installation backend supplies these operations. No IPC exposes this surface.
trait PassphraseStore {
    fn read(&self) -> crate::Result<Option<Zeroizing<Vec<u8>>>>;
    /// Create only: an existing item must never be overwritten, including on a race.
    fn insert(&self, secret: &[u8]) -> crate::Result<()>;
}

fn locked(secret: &[u8]) -> crate::Result<SharedLockedArray> {
    if secret.len() != 32 {
        return Err(denied("Protected installation credential has an invalid length"));
    }
    let mut value =
        sodoken::LockedArray::new(32).map_err(|_| denied("Allocating protected credential"))?;
    value.lock().copy_from_slice(secret);
    Ok(Arc::new(Mutex::new(value)))
}

/// Missing custody for any existing state is an explicit transition/recovery case.
/// Ordinary Open never tries the legacy empty passphrase or issues a replacement.
fn resolve(store: &impl PassphraseStore, root: &Path) -> crate::Result<SharedLockedArray> {
    match root.symlink_metadata() {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err(denied("Installation root is not a real directory"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if let Some(secret) = store.read()? {
        // An existing credential is evidence of a prior initialization attempt,
        // not permission to recreate missing author/database state.
        for relative in [
            "keystore/lair-keystore-config.yaml",
            "keystore/store_file",
            "conductor/databases/db.key",
            "conductor/databases/conductor.db",
        ] {
            let metadata = root.join(relative).symlink_metadata().map_err(|_| denied("Retained credential has missing installation state; explicit recovery is required"))?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(denied("Retained installation state is not a regular file"));
            }
        }
        return locked(&secret);
    }
    if root.try_exists()? && root.read_dir()?.next().is_some() {
        return Err(denied("Existing installation has no accessible protected credential; explicit custody transition or recovery is required"));
    }
    let mut candidate = Zeroizing::new(vec![0; 32]);
    sodoken::random::randombytes_buf(&mut candidate)
        .map_err(|_| denied("Generating installation credential"))?;
    // The platform's insert-only operation serializes competing creators. If it
    // fails, propagate the failure; do not replace a concurrently created item.
    store.insert(&candidate)?;
    let persisted = store
        .read()?
        .ok_or_else(|| denied("New installation credential could not be retrieved"))?;
    if persisted.as_slice() != candidate.as_slice() {
        return Err(denied("Installation credential changed during creation"));
    }
    locked(&persisted)
}

/// Obtain the protected passphrase before opening Lair or conductor databases.
/// The access group is compiled into the approved package, never supplied by IPC
/// or a runtime environment override. macOS validates the signed app and group.
pub fn protected_passphrase(root: &Path) -> crate::Result<SharedLockedArray> {
    #[cfg(target_os = "macos")]
    {
        let store = macos::AppKeychain::for_current_process()?;
        resolve(&store, root)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = root;
        Err(denied("Protected custody is supported only by the approved macOS package"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct MemoryStore {
        secret: RefCell<Option<Vec<u8>>>,
        writes: Cell<usize>,
        fail_read: Cell<bool>,
    }
    impl PassphraseStore for MemoryStore {
        fn read(&self) -> crate::Result<Option<Zeroizing<Vec<u8>>>> {
            if self.fail_read.get() {
                return Err(denied("Access denied"));
            }
            Ok(self.secret.borrow().clone().map(Zeroizing::new))
        }
        fn insert(&self, secret: &[u8]) -> crate::Result<()> {
            if self.secret.borrow().is_some() {
                return Err(denied("Credential already exists"));
            }
            self.writes.set(self.writes.get() + 1);
            *self.secret.borrow_mut() = Some(secret.to_vec());
            Ok(())
        }
    }

    #[test]
    fn fresh_installation_creates_once_and_restart_reuses_the_secret() {
        let root = std::env::temp_dir().join(format!("map-custody-absent-{}", std::process::id()));
        assert!(!root.exists());
        let store = MemoryStore::default();
        let first = resolve(&store, &root).unwrap();
        assert!(!root.exists(), "custody does not provision or mutate the installation");
        assert!(
            resolve(&store, &root).is_err(),
            "interrupted initialization is not fresh provisioning"
        );
        for relative in [
            "keystore/lair-keystore-config.yaml",
            "keystore/store_file",
            "conductor/databases/db.key",
            "conductor/databases/conductor.db",
        ] {
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"fixture").unwrap();
        }
        let reopened = resolve(&store, &root).unwrap();
        assert_eq!(&*first.lock().unwrap().lock(), &*reopened.lock().unwrap().lock());
        assert_eq!(store.writes.get(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_state_or_denied_custody_never_reissues_a_secret() {
        let root =
            std::env::temp_dir().join(format!("map-custody-existing-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("existing-state"), b"preserve").unwrap();
        let store = MemoryStore::default();
        assert!(resolve(&store, &root).is_err());
        store.fail_read.set(true);
        assert!(resolve(&store, &root.join("absent")).is_err());
        assert_eq!(store.writes.get(), 0);
        assert_eq!(std::fs::read(root.join("existing-state")).unwrap(), b"preserve");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_retained_credentials_fail_without_replacement() {
        let store = MemoryStore::default();
        for bytes in [vec![], vec![1; 31], vec![1; 33]] {
            assert!(locked(&bytes).is_err());
        }
        *store.secret.borrow_mut() = Some(vec![1; 32]);
        assert!(resolve(&store, Path::new("unused-absent-installation")).is_err());
        assert_eq!(store.writes.get(), 0);
    }

    #[test]
    fn insert_only_storage_does_not_replace_an_existing_credential() {
        let store = MemoryStore::default();
        store.insert(&[1; 32]).unwrap();
        assert!(store.insert(&[2; 32]).is_err());
        assert!(store.secret.borrow().as_ref().unwrap().as_slice() == [1; 32]);
        assert_eq!(store.writes.get(), 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unsigned_test_process_cannot_construct_the_platform_custody_provider() {
        // This checks code admission only. It does not add/read/delete Keychain items.
        assert!(macos::AppKeychain::for_current_process().is_err());
    }
}
