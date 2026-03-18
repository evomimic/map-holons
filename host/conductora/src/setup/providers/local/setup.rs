use std::sync::Arc;
use crate::config::providers::local::LocalConfig;
use crate::config::StorageProvider;
use crate::setup::common_setup::{create_snapshot_store, register_receptor, serialize_props};
use holons_client::shared_types::base_receptor::BaseReceptor;
use holons_recovery::RecoveryStore;
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
        handle: &AppHandle,
        name: &str,
        local_config: &LocalConfig,
    ) -> anyhow::Result<BaseReceptor> {
        tracing::debug!("[LOCAL SETUP] Building Local storage receptor.");

        let snapshot_store: Option<Arc<dyn RecoveryStore>> =
            create_snapshot_store(handle, local_config, name)
                .await?
                .map(|s| s as Arc<dyn RecoveryStore>);
        let props = serialize_props(local_config);

        Ok(BaseReceptor {
            receptor_id: None,
            receptor_type: "local".to_string(),
            client_handler: None,
            snapshot_store,
            properties: props,
        })
    }

}
