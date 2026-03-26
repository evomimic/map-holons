//use std::sync::Arc;
//use crate::config::providers::ProviderConfig;
use crate::config::providers::local::LocalConfig;
use crate::config::StorageProvider;
use crate::setup::common_setup::{register_receptor, serialize_props};
//use core_types::HolonError;
use holons_client::shared_types::base_receptor::BaseReceptor;
//use holons_recovery::{RecoveryStore, TransactionRecoveryStore};
use tauri::{AppHandle};

pub struct LocalSetup;

impl LocalSetup {
    /// Main setup function for Local integration
    pub async fn setup(
        handle: AppHandle,
        name: &str,
        provider: &StorageProvider,
    ) -> anyhow::Result<()> {
        let StorageProvider::Local(local_cfg) = provider else {
            return Err(anyhow::anyhow!("Invalid storage provider config for Local"));
        };
        let receptor_cfg: BaseReceptor = Self::build_receptor(&handle, name, local_cfg).await?;
        register_receptor(&handle, receptor_cfg).await?;
        Ok(())
    }

    /// Build the receptor configuration for Local storage
    async fn build_receptor(
        _handle: &AppHandle,
        _name: &str,
        local_config: &LocalConfig,
    ) -> anyhow::Result<BaseReceptor> {
        tracing::debug!("[LOCAL SETUP] Building Local storage receptor.");

       // let snapshot_store: Option<Arc<dyn RecoveryStore>> =
      //      create_snapshot_store(handle, local_config, name)
       //         .await?
       //         .map(|s| s as Arc<dyn RecoveryStore>);
        let props = serialize_props(local_config);

        Ok(BaseReceptor {
            receptor_id: None,
            receptor_type: "local".to_string(),
            client_handler: None,
            //snapshot_store,
            properties: props,
        })
    }

}

/*
/// Create a snapshot recovery store for any provider config type that implements `ProviderConfig`.
///
/// - Returns `Ok(None)` if `snapshot_recovery` is not enabled in the config.
/// - Returns `Ok(Some(store))` if a snapshot store was successfully created.
/// - Returns `Err` if the app data directory cannot be resolved or the store cannot be created.
///
/// The database is placed at: `{app_data_dir}/storage/{name}/snapshots.db`
///
/// Blocking I/O (dir creation + SQLite open) is offloaded via `spawn_blocking`.
pub async fn create_snapshot_store<C: ProviderConfig>(
    handle: &AppHandle,
    config: &C,
    name: &str,
) -> Result<Option<Arc<TransactionRecoveryStore>>, HolonError> {
    //if !config.snapshot_recovery() {
    //    tracing::debug!(
     //       "[SNAPSHOT] Skipping snapshot store for '{}' (snapshot_recovery not enabled)",
     //       name
     //   );
     //   return Ok(None);
     //  }

    // Path resolution is non-blocking — do it on the async thread
    let app_data_dir = handle
        .path()
        .app_data_dir()
        .map_err(|e| HolonError::Misc(format!("Failed to resolve app data dir: {}", e)))?;

    let snapshot_dir = app_data_dir.join("storage").join(name);
    let db_path = snapshot_dir.join("snapshots.db");
    tracing::info!("[SNAPSHOT] Creating snapshot store at: {:?}", db_path);

    // Offload blocking filesystem + SQLite open to a dedicated thread
    let store =
        tokio::task::spawn_blocking(move || -> Result<TransactionRecoveryStore, HolonError> {
            std::fs::create_dir_all(&snapshot_dir).map_err(|e| {
                HolonError::Misc(format!("Failed to create snapshot dir {:?}: {}", snapshot_dir, e))
            })?;
            TransactionRecoveryStore::new(&db_path)
        })
        .await
        .map_err(|e| HolonError::Misc(format!("spawn_blocking panicked: {}", e)))??;

    Ok(Some(Arc::new(store)))
}*/
