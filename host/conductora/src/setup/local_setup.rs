use tauri::{AppHandle, Manager};
use holons_client::shared_types::base_receptor::BaseReceptor;
use crate::config::{LocalConfig, StorageProvider};
use crate::setup::receptor_config_registry::ReceptorConfigRegistry;

pub struct LocalSetup;

impl LocalSetup {
    /// Main setup function for Local integration
    pub async fn setup(handle: AppHandle, provider: &StorageProvider) -> anyhow::Result<()> {
         let StorageProvider::Local(local_cfg) = provider else {
            return Err(anyhow::anyhow!("Invalid storage provider config for Local"));
        };
        let receptor_cfg: BaseReceptor = Self::build_receptor(local_cfg).await?;
        Self::register_receptor(&handle, receptor_cfg).await?;

        Ok(())
    }

    /// Build the receptor configuration for Local storage
    async fn build_receptor(
        config: &LocalConfig
    ) -> anyhow::Result<BaseReceptor> {
        tracing::debug!("[LOCAL SETUP] Building Local storage receptor.");

        // Dynamically collect all properties from the local config
            let props = match serde_json::to_value(config)? {
                serde_json::Value::Object(map) => {
                    map.into_iter()
                        .map(|(k, v)| {
                            let value_str = match v {
                                serde_json::Value::String(s) => s,
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                serde_json::Value::Null => String::new(),
                                _ => v.to_string(),
                            };
                            (k, value_str)
                        })
                        .collect::<std::collections::HashMap<String, String>>()
                }
                _ => std::collections::HashMap::new(),
            };

            return Ok(BaseReceptor {
                receptor_id: None,
                receptor_type: "local".to_string(),
                client_handler: None,
                properties: props,
            });
        }


    /// Register the Local storage receptor
        async fn register_receptor(
            handle: &AppHandle,
            receptor_cfg: BaseReceptor,
        ) -> anyhow::Result<()> {
            // Get the registry from app state and register the new config
            let registry = handle.state::<ReceptorConfigRegistry>();
            registry.register(receptor_cfg);
            Ok(())
        }
}