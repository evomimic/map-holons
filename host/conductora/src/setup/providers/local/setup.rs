use crate::config::providers::local::LocalConfig;
use crate::config::providers::ProviderConfig;
use crate::config::StorageProvider;
use crate::runtime::init_runtime::RecoveryReceptorState;
use crate::setup::common_setup::serialize_props;
use client_shared_types::base_receptor::{BaseReceptor, ReceptorType};
use recovery_receptor::local_recovery_receptor::LocalRecoveryReceptor;
use recovery_receptor::{RecoveryStore, TransactionRecoveryStore};
use std::sync::Arc;
use tauri::{AppHandle, Manager}; // alias lives in runtime, not here

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
            Self::build_recovery_receptor(&handle, name, local_cfg).await?;
        } else {
            return Err(anyhow::anyhow!(
                "Local storage '{}' enabled without 'recovery' feature: Registering a non-recovery receptor is currently not allowed as we have not defined other local receptors.",name));
        }
        Ok(())
    }

    /// Build the receptor configuration for Local storage
    async fn build_recovery_receptor(
        handle: &AppHandle,
        name: &str,
        local_config: &LocalConfig,
    ) -> anyhow::Result<()> {
        tracing::info!("[LOCAL SETUP] Session feature enabled for Local storage.");
        let snapshot_store = create_snapshot_store(handle, local_config, name).await?;
        // continue with receptor config creation as normal
        let props = serialize_props(local_config);
        let receptor = Arc::new(LocalRecoveryReceptor::from_base(
            BaseReceptor {
                // clean BaseReceptor (no client_handler)
                receptor_id: name.to_string(),
                receptor_type: ReceptorType::LocalRecovery,
                properties: props.clone(),
            },
            Arc::clone(&snapshot_store),
        ));
        if let Some(state) = handle.try_state::<RecoveryReceptorState>() {
            if let Ok(mut guard) = state.write() {
                *guard = Some(receptor);
                tracing::info!("[LOCAL SETUP] LocalRecoveryReceptor stored in typed state.");
            }
        }
        Ok(())

        // Ok(BaseReceptor {
        //     receptor_id: name.to_string(),
        //     receptor_type: ReceptorType::LocalRecovery,
        //     properties: props,
        // })
    }
}

/// Create a snapshot recovery store for any provider config type that implements `ProviderConfig`.
///
/// - Returns `Ok(None)` if `snapshot_recovery` is not enabled in the config.
/// - Returns `Ok(Some(store))` if a snapshot store was successfully created.
/// - Returns `Err` if the app data directory cannot be resolved or the store cannot be created.
///
/// The database is placed at:
/// - production: `{app_data_dir}/storage/{name}/snapshots.db`
/// - HC dev mode: `/tmp/conductora_dev/local_recovery/{name}/snapshots.db`
///
/// The dev path mirrors the Holochain conductor dev-data convention, so `clean:hc:deep`
/// wipes both automatically.
///
/// Blocking I/O (dir creation + SQLite open) is offloaded via `spawn_blocking`.
pub async fn create_snapshot_store<C: ProviderConfig>(
    handle: &AppHandle,
    _config: &C,
    name: &str,
) -> Result<Arc<TransactionRecoveryStore>, anyhow::Error> {
    // Path resolution is non-blocking — do it on the async thread
    let snapshot_dir = if crate::env::hc_dev_mode_enabled() {
        std::path::PathBuf::from("/tmp/conductora_dev").join(name)
    } else {
        let app_data_dir = handle
            .path()
            .app_data_dir()
            .map_err(|e| anyhow::anyhow!("Failed to resolve app data dir: {}", e))?;
        app_data_dir.join("storage").join(name)
    };
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
