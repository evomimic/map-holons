//! Explicit offline roll-forward of two credential wrappers, never MAP Commit.
use super::{denied, locked, prepare_passphrase_transition, PassphraseStore};
use crate::installation_lock::InstallationLease;
use lair_keystore_api::{
    config::LairServerConfigInner, dependencies::sodoken, types::SharedLockedArray,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};
use zeroize::Zeroizing;

const PENDING: &str = ".credential-cutover";
const LAIR: &str = "keystore/lair-keystore-config.yaml";
const DATABASE: &str = "conductor/databases/db.key";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    version: u8,
    root: std::path::PathBuf,
    lair: String,
    database: String,
    previous_lair: String,
    previous_database: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    CredentialRetained,
    RecoveryPrepared,
    LairInstalled,
    DatabaseInstalled,
}

fn regular_read(root: &Path, name: &str) -> crate::Result<Vec<u8>> {
    // Refuse symlink traversal throughout the supported fixed layout.
    let mut path = root.to_path_buf();
    for component in Path::new(name).components() {
        path.push(component);
        let metadata = path.symlink_metadata()?;
        if metadata.file_type().is_symlink() {
            return Err(denied("Credential path contains a symlink"));
        }
    }
    if !path.is_file() {
        return Err(denied("Credential path is not a regular file"));
    }
    Ok(std::fs::read(path)?)
}

fn write_new(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn sync_directory(path: &Path) -> crate::Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn replace(root: &Path, name: &str, bytes: &[u8]) -> crate::Result<()> {
    let target = root.join(name);
    let scratch = target.with_extension("custody-new");
    match scratch.symlink_metadata() {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => std::fs::remove_file(&scratch)?,
        Ok(_) => return Err(denied("Unsafe credential replacement scratch path")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    write_new(&scratch, bytes)?;
    std::fs::rename(scratch, &target)?;
    sync_directory(target.parent().unwrap())
}

fn recovery_key(secret: &[u8]) -> crate::Result<sodoken::SizedLockedArray<32>> {
    let mut master = sodoken::SizedLockedArray::<32>::new()?;
    if secret.len() != 32 {
        return Err(denied("Invalid retained custody credential"));
    }
    master.lock().copy_from_slice(secret);
    let mut derived = sodoken::SizedLockedArray::<32>::new()?;
    sodoken::kdf::derive_from_key(&mut *derived.lock(), 1, b"WrapMove", &master.lock())?;
    Ok(derived)
}

fn seal(value: &Replacement, secret: &[u8]) -> crate::Result<Vec<u8>> {
    let mut key = recovery_key(secret)?;
    let plaintext = Zeroizing::new(serde_json::to_vec(value)?);
    let mut nonce = [0; sodoken::secretbox::XSALSA_NONCEBYTES];
    sodoken::random::randombytes_buf(&mut nonce)?;
    let mut ciphertext = vec![0; plaintext.len() + sodoken::secretbox::XSALSA_MACBYTES];
    sodoken::secretbox::xsalsa_easy(&mut ciphertext, &nonce, &plaintext, &key.lock())?;
    let mut bytes = b"MAPCUST1".to_vec();
    bytes.extend(nonce);
    bytes.extend(ciphertext);
    Ok(bytes)
}

fn unseal(bytes: &[u8], secret: &[u8]) -> crate::Result<Replacement> {
    if bytes.len() < 8 + 24 + 16 || &bytes[..8] != b"MAPCUST1" {
        return Err(denied("Invalid credential recovery envelope"));
    }
    let mut key = recovery_key(secret)?;
    let nonce: &[u8; 24] = bytes[8..32].try_into().unwrap();
    let mut plaintext = Zeroizing::new(vec![0; bytes.len() - 32 - 16]);
    sodoken::secretbox::xsalsa_open_easy(&mut plaintext, &bytes[32..], nonce, &key.lock())
        .map_err(|_| denied("Credential recovery authentication failed"))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

async fn validate_wrappers(
    root: &Path,
    lair: &str,
    database: &str,
    secret: SharedLockedArray,
) -> crate::Result<()> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    // Upstream DbKey::load slices this fixed-size format; reject truncation first.
    if URL_SAFE_NO_PAD
        .decode(database)
        .map_err(|_| denied("Invalid database wrapper encoding"))?
        .len()
        != 88
    {
        return Err(denied("Invalid database wrapper size"));
    }
    let config =
        LairServerConfigInner::from_bytes(lair.as_bytes()).map_err(crate::Error::LairError)?;
    if config.store_file.canonicalize()? != root.join("keystore/store_file") {
        return Err(denied("Lair configuration points outside the supported installation"));
    }
    super::rewrap_lair_config(&config, secret.clone(), secret.clone())
        .await
        .map_err(crate::Error::LairError)?;
    holochain_data::DbKey::load(database.to_owned(), secret).await?;
    Ok(())
}

async fn execute(
    store: &impl PassphraseStore,
    root: &Path,
    old: Option<SharedLockedArray>,
    mut after: impl FnMut(Step) -> crate::Result<()>,
) -> crate::Result<()> {
    let _lease = InstallationLease::acquire(root)?;
    let root = root.canonicalize()?;
    // Validate the complete fixed layout before issuing a credential or rewriting anything.
    let current_lair = String::from_utf8(regular_read(&root, LAIR)?)
        .map_err(|_| denied("Invalid Lair configuration encoding"))?;
    let current_database = String::from_utf8(regular_read(&root, DATABASE)?)
        .map_err(|_| denied("Invalid database wrapper encoding"))?;
    regular_read_metadata(&root, "keystore/store_file")?;
    regular_read_metadata(&root, "conductor/databases/conductor.db")?;
    let existing = store.read()?;
    let pending = match root.join(PENDING).symlink_metadata() {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => true,
        Ok(_) => return Err(denied("Unsafe credential recovery path")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };
    if pending && existing.is_none() {
        return Err(denied(
            "Pending transition has no retained credential; recovery must not reissue it",
        ));
    }
    let secret = match existing {
        Some(value) => value,
        None => {
            let old = old
                .clone()
                .ok_or_else(|| denied("Initial cutover requires the existing passphrase"))?;
            validate_wrappers(&root, &current_lair, &current_database, old).await?;
            let mut value = Zeroizing::new(vec![0; 32]);
            sodoken::random::randombytes_buf(&mut value)?;
            value
        }
    };
    let new = locked(&secret)?;
    if !pending
        && validate_wrappers(&root, &current_lair, &current_database, new.clone()).await.is_ok()
    {
        return Ok(()); // Already installed: do not issue, rotate or repeat the cutover.
    }
    let replacement = if pending {
        let value = unseal(&regular_read(&root, PENDING)?, &secret)?;
        if value.version != 1 || value.root != root {
            return Err(denied("Credential recovery belongs to another installation/version"));
        }
        value
    } else {
        let old = old.ok_or_else(|| {
            denied("No prepared transition exists; explicit initial cutover is required")
        })?;
        validate_wrappers(&root, &current_lair, &current_database, old.clone()).await?;
        let wrappers = prepare_passphrase_transition(&root, old, new.clone()).await?;
        let value = Replacement {
            version: 1,
            root: root.clone(),
            lair: wrappers.lair_configuration,
            database: wrappers.database_key_file,
            previous_lair: sha256::digest(current_lair.as_bytes()),
            previous_database: sha256::digest(current_database.as_bytes()),
        };
        if store.read()?.is_none() {
            store.insert(&secret)?;
        }
        let retained = store.read()?.ok_or_else(|| denied("Cutover credential readback failed"))?;
        if retained.as_slice() != secret.as_slice() {
            return Err(denied("Cutover credential changed"));
        }
        after(Step::CredentialRetained)?;
        let envelope = seal(&value, &secret)?;
        // Publish only a fully flushed envelope. A pre-publication crash leaves
        // the original pair untouched and can repeat preparation with retained custody.
        replace(&root, ".credential-cutover-prepared", &envelope)?;
        std::fs::rename(root.join(".credential-cutover-prepared"), root.join(PENDING))?;
        sync_directory(&root)?;
        after(Step::RecoveryPrepared)?;
        value
    };
    validate_wrappers(&root, &replacement.lair, &replacement.database, new).await?;
    for (current, previous, desired) in [
        (&current_lair, &replacement.previous_lair, &replacement.lair),
        (&current_database, &replacement.previous_database, &replacement.database),
    ] {
        if current != desired && sha256::digest(current.as_bytes()) != *previous {
            return Err(denied(
                "Installation wrappers changed outside this transition; refusing overwrite",
            ));
        }
    }
    replace(&root, LAIR, replacement.lair.as_bytes())?;
    after(Step::LairInstalled)?;
    replace(&root, DATABASE, replacement.database.as_bytes())?;
    after(Step::DatabaseInstalled)?;
    std::fs::remove_file(root.join(PENDING))?;
    sync_directory(&root)?;
    Ok(())
}

fn regular_read_metadata(root: &Path, name: &str) -> crate::Result<()> {
    let path = root.join(name);
    let metadata = path.symlink_metadata()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(denied("Incomplete retained installation"));
    }
    Ok(())
}

/// Explicit backend-only offline cutover or resumption. No IPC or startup calls it.
/// Stop older/non-cooperating binaries and independently opened stores first.
/// External legacy backups require an explicit protected retention disposition.
#[cfg(target_os = "macos")]
pub async fn transition_existing_installation(
    root: &Path,
    old: Option<SharedLockedArray>,
) -> crate::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let store = super::macos::AppKeychain::for_current_process()?;
        execute(&store, root, old, |_| Ok(())).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (root, old);
        Err(denied("Protected cutover requires the approved macOS package"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::ensure_no_pending_transition;
    use super::super::tests::MemoryStore;
    use super::*;
    use lair_keystore_api::in_proc_keystore::InProcKeystore;
    use std::sync::Arc;

    struct Fixture {
        root: std::path::PathBuf,
        author: lair_keystore_api::encoding_types::Ed25519PubKey,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    impl Fixture {
        async fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "map-cutover-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(root.join("keystore")).unwrap();
            std::fs::create_dir_all(root.join("conductor/databases")).unwrap();
            let old = crate::vec_to_locked(vec![]);
            let config = lair_keystore_api::dependencies::hc_seed_bundle::PwHashLimits::Minimum
                .with_exec(|| LairServerConfigInner::new(root.join("keystore"), old.clone()))
                .await
                .unwrap();
            let keystore = InProcKeystore::new(
                Arc::new(config.clone()),
                lair_keystore::create_sql_pool_factory(&config.store_file, &config.database_salt),
                old.clone(),
            )
            .await
            .unwrap();
            let client = keystore.new_client().await.unwrap();
            let author = client
                .new_seed("retained-author".into(), None, false)
                .await
                .unwrap()
                .ed25519_pub_key;
            let key = holochain_data::DbKey::generate(old).await.unwrap();
            let db = holochain_data::open_db(
                &root.join("conductor/databases"),
                holochain_data::kind::Conductor,
                holochain_data::HolochainDataConfig::default().with_key(key.clone()),
            )
            .await
            .unwrap();
            db.set_conductor_tag("retained-content").await.unwrap();
            db.pool().close().await;
            std::fs::write(root.join(LAIR), config.to_string()).unwrap();
            std::fs::write(root.join(DATABASE), key.locked).unwrap();
            Self { root: root.canonicalize().unwrap(), author }
        }
        async fn verify(&self, store: &MemoryStore) {
            let new = locked(&store.read().unwrap().unwrap()).unwrap();
            let config =
                LairServerConfigInner::from_bytes(&std::fs::read(self.root.join(LAIR)).unwrap())
                    .unwrap();
            let keystore = InProcKeystore::new(
                Arc::new(config.clone()),
                lair_keystore::create_sql_pool_factory(&config.store_file, &config.database_salt),
                new.clone(),
            )
            .await
            .unwrap();
            let client = keystore.new_client().await.unwrap();
            let challenge: Arc<[u8]> = Arc::from(&b"preserved author"[..]);
            let signature =
                client.sign_by_pub_key(self.author.clone(), None, challenge.clone()).await.unwrap();
            assert!(self.author.verify_detached(signature, challenge).await);
            let wrapper = std::fs::read_to_string(self.root.join(DATABASE)).unwrap();
            assert!(holochain_data::DbKey::load(wrapper.clone(), crate::vec_to_locked(vec![]))
                .await
                .is_err());
            let key = holochain_data::DbKey::load(wrapper, new).await.unwrap();
            let db = holochain_data::open_db(
                &self.root.join("conductor/databases"),
                holochain_data::kind::Conductor,
                holochain_data::HolochainDataConfig::default().with_key(key),
            )
            .await
            .unwrap();
            assert_eq!(
                db.as_ref().get_conductor_tag().await.unwrap().as_deref(),
                Some("retained-content")
            );
            db.pool().close().await;
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn interrupted_cutover_rolls_forward_without_reissuing_or_replacing_identity() {
        for stop in [
            Step::CredentialRetained,
            Step::RecoveryPrepared,
            Step::LairInstalled,
            Step::DatabaseInstalled,
        ] {
            let fixture = Fixture::new().await;
            let store = MemoryStore::default();
            let before_lair = std::fs::read(fixture.root.join(LAIR)).unwrap();
            let before_database = std::fs::read(fixture.root.join(DATABASE)).unwrap();
            let before_seed_store =
                std::fs::read(fixture.root.join("keystore/store_file")).unwrap();
            assert!(execute(&store, &fixture.root, Some(crate::vec_to_locked(vec![])), |step| {
                if step == stop {
                    Err(denied("injected interruption"))
                } else {
                    Ok(())
                }
            })
            .await
            .is_err());
            let retained = store.read().unwrap().unwrap();
            if stop == Step::CredentialRetained {
                assert_eq!(std::fs::read(fixture.root.join(LAIR)).unwrap(), before_lair);
                assert_eq!(std::fs::read(fixture.root.join(DATABASE)).unwrap(), before_database);
            } else {
                assert!(ensure_no_pending_transition(&fixture.root).is_err());
                assert!(super::super::resolve(&store, &fixture.root).is_err());
            }
            execute(
                &store,
                &fixture.root,
                if stop == Step::CredentialRetained {
                    Some(crate::vec_to_locked(vec![]))
                } else {
                    None
                },
                |_| Ok(()),
            )
            .await
            .unwrap();
            assert!(store.read().unwrap().unwrap().as_slice() == retained.as_slice());
            assert_eq!(
                std::fs::read(fixture.root.join("keystore/store_file")).unwrap(),
                before_seed_store
            );
            ensure_no_pending_transition(&fixture.root).unwrap();
            fixture.verify(&store).await;
            let pair = (
                std::fs::read(fixture.root.join(LAIR)).unwrap(),
                std::fs::read(fixture.root.join(DATABASE)).unwrap(),
            );
            execute(&store, &fixture.root, None, |_| {
                panic!("completed transition must be idempotent")
            })
            .await
            .unwrap();
            assert_eq!(std::fs::read(fixture.root.join(LAIR)).unwrap(), pair.0);
            assert_eq!(std::fs::read(fixture.root.join(DATABASE)).unwrap(), pair.1);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wrong_legacy_passphrase_and_changed_source_refuse_replacement() {
        let fixture = Fixture::new().await;
        let store = MemoryStore::default();
        let before = std::fs::read(fixture.root.join(DATABASE)).unwrap();
        assert!(execute(
            &store,
            &fixture.root,
            Some(crate::vec_to_locked(vec![3; 32])),
            |_| Ok(())
        )
        .await
        .is_err());
        assert!(store.read().unwrap().is_none());
        assert_eq!(std::fs::read(fixture.root.join(DATABASE)).unwrap(), before);
        assert!(execute(&store, &fixture.root, Some(crate::vec_to_locked(vec![])), |step| {
            if step == Step::LairInstalled {
                Err(denied("stop"))
            } else {
                Ok(())
            }
        })
        .await
        .is_err());
        let changed = b"outside modification";
        std::fs::write(fixture.root.join(DATABASE), changed).unwrap();
        assert!(execute(&store, &fixture.root, None, |_| Ok(())).await.is_err());
        assert_eq!(std::fs::read(fixture.root.join(DATABASE)).unwrap(), changed);
        assert!(ensure_no_pending_transition(&fixture.root).is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn active_runtime_tampered_recovery_and_missing_custody_refuse_writes() {
        let fixture = Fixture::new().await;
        let store = MemoryStore::default();
        let lease = InstallationLease::acquire(&fixture.root).unwrap();
        assert!(execute(&store, &fixture.root, Some(crate::vec_to_locked(vec![])), |_| Ok(()))
            .await
            .is_err());
        assert!(store.read().unwrap().is_none());
        drop(lease);
        assert!(execute(&store, &fixture.root, Some(crate::vec_to_locked(vec![])), |step| {
            if step == Step::RecoveryPrepared {
                Err(denied("stop"))
            } else {
                Ok(())
            }
        })
        .await
        .is_err());
        let before = (
            std::fs::read(fixture.root.join(LAIR)).unwrap(),
            std::fs::read(fixture.root.join(DATABASE)).unwrap(),
        );
        assert!(execute(
            &MemoryStore::default(),
            &fixture.root,
            Some(crate::vec_to_locked(vec![])),
            |_| Ok(())
        )
        .await
        .is_err());
        let marker = fixture.root.join(PENDING);
        let mut bytes = std::fs::read(&marker).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        std::fs::write(&marker, bytes).unwrap();
        assert!(execute(&store, &fixture.root, None, |_| Ok(())).await.is_err());
        assert_eq!(std::fs::read(fixture.root.join(LAIR)).unwrap(), before.0);
        assert_eq!(std::fs::read(fixture.root.join(DATABASE)).unwrap(), before.1);
    }
}
