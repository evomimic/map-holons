use std::sync::Arc;
use crate::config::providers::ProviderConfig;
use crate::config::providers::local::LocalConfig;
use crate::config::StorageProvider;
use crate::setup::common_setup::{register_receptor, serialize_props};
//use client_shared_types::base_receptor::{BaseReceptor, ReceptorType};
use holons_client::shared_types::base_receptor::{BaseReceptor, ReceptorType};
use recovery_receptor::{RecoveryStore, TransactionRecoveryStore};
use tauri::{AppHandle, Manager};

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
        //let t_setup = std::time::Instant::now();
        let is_recovery = local_cfg.features.iter().any(|f| f == "recovery");
        if is_recovery {
            let receptor_cfg: BaseReceptor = Self::build_recovery_receptor(&handle, name, local_cfg).await?;
            register_receptor(&handle, receptor_cfg).await?;
        }
        Ok(())
    }

    /// Build the receptor configuration for Local storage
    async fn build_recovery_receptor(
        handle: &AppHandle,
        name: &str,
        local_config: &LocalConfig,
    ) -> anyhow::Result<BaseReceptor> {
        tracing::info!("[LOCAL SETUP] Recovery feature enabled for Local storage.");
        let snapshot_store = create_snapshot_store(handle, local_config, name).await?;
        
        // continue with receptor config creation as normal
        let props = serialize_props(local_config);
    
        Ok(BaseReceptor {
            receptor_id: name.to_string(),
            receptor_type: ReceptorType::LocalRecovery,
            client_handler: Some(snapshot_store as Arc<dyn std::any::Any + Send + Sync>),
            properties: props,
        })
    }

}


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
    _config: &C,
    name: &str,
) -> Result<Arc<TransactionRecoveryStore>, anyhow::Error> {

    // Path resolution is non-blocking — do it on the async thread
    let app_data_dir = handle
        .path()
        .app_data_dir()
        .map_err(|e| anyhow::anyhow!("Failed to resolve app data dir: {}", e))?;

    let snapshot_dir = app_data_dir.join("storage").join(name);
    let db_path = snapshot_dir.join("snapshots.db");
    tracing::info!("[SNAPSHOT] Creating snapshot store at: {:?}", db_path);

    // Offload blocking filesystem + SQLite open to a dedicated thread
    let store =
        tokio::task::spawn_blocking(move || -> Result<TransactionRecoveryStore, anyhow::Error> {
            std::fs::create_dir_all(&snapshot_dir).map_err(|e| {
                anyhow::anyhow!("Failed to create snapshot dir {:?}: {}", snapshot_dir, e)
            })?;
            TransactionRecoveryStore::new(&db_path).map_err(|e| {
                anyhow::anyhow!("Failed to create TransactionRecoveryStore at {:?}: {}", db_path, e)
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {}", e))??;

    Ok(Arc::new(store))
}

//helpers
//fn generate_receptor_id(props: HashMap<String, String>) -> Result<String, Box<dyn std::error::Error>> {
//    let json = serde_json::to_string(&props)?;
 //   Ok(hex::encode(Sha256::digest(json.as_bytes())))
//}
