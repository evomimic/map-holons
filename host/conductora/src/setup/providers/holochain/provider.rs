use async_trait::async_trait;
use tauri::AppHandle;
use tauri_plugin_holochain::HolochainExt;

use crate::config::StorageProvider;
use crate::setup::provider_integration::ProviderIntegration;
use crate::setup::window_setup::ProviderWindowSetup;

use super::plugins::holochain_plugin;
use super::setup::{HolochainSetup, HolochainWindowSetup};

pub struct HolochainProvider;

impl HolochainProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProviderIntegration for HolochainProvider {
    fn provider_type(&self) -> &'static str {
        "holochain"
    }

    fn setup_event(&self) -> Option<&'static str> {
        Some("holochain://setup-completed")
    }

    fn setup_failed_event(&self) -> Option<&'static str> {
        Some("holochain://setup-failed")
    }

    fn is_ready(&self, handle: &AppHandle) -> bool {
        handle.holochain().is_ok()
    }

    fn apply_plugins(
        &self,
        mut builder: tauri::Builder<tauri::Wry>,
        provider_key: &str,
        provider: &StorageProvider,
    ) -> anyhow::Result<tauri::Builder<tauri::Wry>> {
        let StorageProvider::Holochain(cfg) = provider else {
            tracing::error!("[PLUGIN MANAGER] Invalid storage provider config for Holochain plugin");
            return Err(anyhow::anyhow!("Invalid storage provider config for Holochain plugin"));
        };

        match holochain_plugin(provider_key, cfg) {
            Ok(plugin) => {
                tracing::debug!("[PLUGIN MANAGER] Loading Holochain plugin");
                builder = builder.plugin(plugin);
            }
            Err(e) => {
                tracing::error!("[PLUGIN MANAGER] Failed to load Holochain plugin: {}", e);
                return Err(anyhow::anyhow!("Failed to load Holochain plugin: {}", e));
            }
        }
        Ok(builder)
    }

    async fn setup(
        &self,
        handle: AppHandle,
        name: &str,
        provider: &StorageProvider,
    ) -> anyhow::Result<()> {
        HolochainSetup::setup(handle, name, provider).await
    }

    fn supports_window(&self) -> bool {
        true
    }

    async fn create_window(
        &self,
        handle: &AppHandle,
        _provider_name: &str,
        provider: &StorageProvider,
    ) -> anyhow::Result<()> {
        let StorageProvider::Holochain(cfg) = provider else {
            return Err(anyhow::anyhow!("Invalid storage provider config for Holochain"));
        };
        let setup = HolochainWindowSetup;
        setup.create_window(handle, &cfg.app_id).await
    }
}
