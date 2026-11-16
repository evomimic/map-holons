//! Version-bounded adapter for Lair 0.7's existing wrapping algorithm.
//! It transforms encrypted configuration in memory; it never writes a keystore.
use lair_keystore_api::{
    config::LairServerConfigInner, dependencies::sodoken, encoding_types::SecretDataSized,
    types::SharedLockedArray, LairResult,
};
use std::sync::{Arc, Mutex};

/// Encrypted replacement metadata prepared for an explicit offline transition.
/// No secret, filesystem mutation or authority is represented by this result.
/// Applying it still requires exclusive shutdown, recoverable two-file installation,
/// protected backup handling and signed-package custody verification.
pub struct PreparedCredentialWrappers {
    pub lair_configuration: String,
    pub database_key_file: String,
}

impl std::fmt::Debug for PreparedCredentialWrappers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedCredentialWrappers").finish_non_exhaustive()
    }
}

/// Prepare replacements without writing or launching anything in the source store.
/// A caller must not treat preparation as a completed custody transition.
pub async fn prepare_passphrase_transition(
    root: &std::path::Path,
    old: SharedLockedArray,
    new: SharedLockedArray,
) -> crate::Result<PreparedCredentialWrappers> {
    let config = LairServerConfigInner::from_bytes(&std::fs::read(
        root.join("keystore/lair-keystore-config.yaml"),
    )?)
    .map_err(crate::Error::LairError)?;
    let database = holochain_data::DbKey::load(
        std::fs::read_to_string(root.join("conductor/databases/db.key"))?,
        old.clone(),
    )
    .await?;
    let lair =
        rewrap_lair_config(&config, old, new.clone()).await.map_err(crate::Error::LairError)?;
    let database = database.rewrap(new).await?;
    Ok(PreparedCredentialWrappers {
        lair_configuration: lair.to_string(),
        database_key_file: database.locked,
    })
}

async fn wrapping_keys(
    config: &LairServerConfigInner,
    passphrase: SharedLockedArray,
) -> LairResult<(
    Arc<Mutex<sodoken::SizedLockedArray<32>>>,
    Arc<Mutex<sodoken::SizedLockedArray<32>>>,
)> {
    let salt = config.runtime_secrets_salt.cloned_inner();
    let ops = config.runtime_secrets_ops_limit;
    let memory = config.runtime_secrets_mem_limit;
    let mut pre = tokio::task::spawn_blocking(move || -> LairResult<_> {
        let mut hash = sodoken::SizedLockedArray::<64>::new()?;
        sodoken::blake2b::blake2b_hash(
            &mut *hash.lock(),
            &passphrase.lock().unwrap().lock(),
            None,
        )?;
        let mut pre = sodoken::SizedLockedArray::<32>::new()?;
        sodoken::argon2::blocking_argon2id(&mut *pre.lock(), &*hash.lock(), &salt, ops, memory)?;
        Ok(pre)
    })
    .await
    .map_err(one_err::OneErr::new)??;
    let mut context = sodoken::SizedLockedArray::<32>::new()?;
    let mut identity = sodoken::SizedLockedArray::<32>::new()?;
    sodoken::kdf::derive_from_key(&mut *context.lock(), 42, b"CtxSecKy", &pre.lock())?;
    sodoken::kdf::derive_from_key(&mut *identity.lock(), 142, b"IdnSecKy", &pre.lock())?;
    Ok((Arc::new(Mutex::new(context)), Arc::new(Mutex::new(identity))))
}

/// Prepare new encrypted wrappers around the same Lair database key/server seed.
/// Preserves database salt, connection identity, paths, KDF settings and seed store.
/// This is not key rotation, completed migration or a credential recovery API.
pub async fn rewrap_lair_config(
    config: &LairServerConfigInner,
    old: SharedLockedArray,
    new: SharedLockedArray,
) -> LairResult<LairServerConfigInner> {
    let (old_context, old_identity) = wrapping_keys(config, old).await?;
    let context =
        Arc::new(Mutex::new(config.runtime_secrets_context_key.decrypt(old_context).await?));
    let identity =
        Arc::new(Mutex::new(config.runtime_secrets_id_seed.decrypt(old_identity).await?));
    let (new_context, new_identity) = wrapping_keys(config, new).await?;
    let mut changed = config.clone();
    changed.runtime_secrets_context_key = SecretDataSized::encrypt(new_context, context).await?;
    changed.runtime_secrets_id_seed = SecretDataSized::encrypt(new_identity, identity).await?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn prepared_transition_reopens_existing_lair_seed_and_database_content() {
        use lair_keystore_api::in_proc_keystore::InProcKeystore;
        let root = Fixture(std::env::temp_dir().join(format!(
            "map-rewrap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        let db_root = root.0.join("conductor/databases");
        std::fs::create_dir_all(&db_root).unwrap();
        std::fs::create_dir_all(root.0.join("keystore")).unwrap();
        let old = crate::vec_to_locked(vec![]);
        let new = crate::vec_to_locked(vec![9; 32]);
        let config = lair_keystore_api::dependencies::hc_seed_bundle::PwHashLimits::Minimum
            .with_exec(|| LairServerConfigInner::new(root.0.join("keystore"), old.clone()))
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
        let agent = client.new_seed("existing-agent".into(), None, false).await.unwrap();
        let key = holochain_data::DbKey::generate(old.clone()).await.unwrap();
        let db = holochain_data::open_db(
            &db_root,
            holochain_data::kind::Conductor,
            holochain_data::HolochainDataConfig::default().with_key(key.clone()),
        )
        .await
        .unwrap();
        db.set_conductor_tag("preserved-installation").await.unwrap();
        db.pool().close().await;
        let config_path = root.0.join("keystore/lair-keystore-config.yaml");
        let key_path = db_root.join("db.key");
        std::fs::write(&config_path, config.to_string()).unwrap();
        std::fs::write(&key_path, &key.locked).unwrap();
        let before_config = std::fs::read(&config_path).unwrap();
        let before_key = std::fs::read(&key_path).unwrap();
        let prepared =
            prepare_passphrase_transition(&root.0, old.clone(), new.clone()).await.unwrap();
        assert_eq!(std::fs::read(&config_path).unwrap(), before_config);
        assert_eq!(std::fs::read(&key_path).unwrap(), before_key);
        assert!(!format!("{prepared:?}").contains(&prepared.database_key_file));
        assert!(prepare_passphrase_transition(
            &root.0,
            crate::vec_to_locked(vec![4; 32]),
            new.clone()
        )
        .await
        .is_err());
        let changed =
            LairServerConfigInner::from_bytes(prepared.lair_configuration.as_bytes()).unwrap();
        assert_eq!(changed.get_server_pub_key().unwrap(), config.get_server_pub_key().unwrap());
        let reopened = InProcKeystore::new(
            Arc::new(changed),
            lair_keystore::create_sql_pool_factory(&config.store_file, &config.database_salt),
            new.clone(),
        )
        .await
        .unwrap();
        let client = reopened.new_client().await.unwrap();
        let challenge: Arc<[u8]> = Arc::from(&b"same author after custody transition"[..]);
        let signature = client
            .sign_by_pub_key(agent.ed25519_pub_key.clone(), None, challenge.clone())
            .await
            .unwrap();
        assert!(agent.ed25519_pub_key.verify_detached(signature, challenge).await);
        let rewrapped =
            holochain_data::DbKey::load(prepared.database_key_file.clone(), new).await.unwrap();
        assert_eq!(&*rewrapped.key.lock().unwrap().lock(), &*key.key.lock().unwrap().lock());
        assert_eq!(&*rewrapped.salt.lock().unwrap().lock(), &*key.salt.lock().unwrap().lock());
        assert!(holochain_data::DbKey::load(prepared.database_key_file, old).await.is_err());
        let db = holochain_data::open_db(
            &db_root,
            holochain_data::kind::Conductor,
            holochain_data::HolochainDataConfig::default().with_key(rewrapped),
        )
        .await
        .unwrap();
        assert_eq!(
            db.as_ref().get_conductor_tag().await.unwrap().as_deref(),
            Some("preserved-installation")
        );
        db.pool().close().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wrapping_changes_preserve_lair_identity_and_reject_the_old_password() {
        let old = crate::vec_to_locked(vec![]);
        let new = crate::vec_to_locked(vec![7; 32]);
        let root = Fixture(std::env::temp_dir().join(format!(
            "map-lair-wrap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        std::fs::create_dir_all(&root.0).unwrap();
        let config = lair_keystore_api::dependencies::hc_seed_bundle::PwHashLimits::Minimum
            .with_exec(|| LairServerConfigInner::new(&root.0, old.clone()))
            .await
            .unwrap();
        let changed = rewrap_lair_config(&config, old.clone(), new.clone()).await.unwrap();
        assert_eq!(config.get_server_pub_key().unwrap(), changed.get_server_pub_key().unwrap());
        assert_eq!(config.database_salt, changed.database_salt);
        assert_eq!(config.store_file, changed.store_file);
        assert_eq!(config.connection_url, changed.connection_url);
        let (a, _) = wrapping_keys(&config, old.clone()).await.unwrap();
        let (b, _) = wrapping_keys(&changed, new.clone()).await.unwrap();
        let mut a = config.runtime_secrets_context_key.decrypt(a).await.unwrap();
        let mut b = changed.runtime_secrets_context_key.decrypt(b).await.unwrap();
        assert_eq!(&*a.lock(), &*b.lock());
        assert!(rewrap_lair_config(&changed, old, new).await.is_err());
    }
}
