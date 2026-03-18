use async_trait::async_trait;
use tauri::AppHandle;

use crate::config::providers::holochain::holochain_plugin;
use crate::config::StorageProvider;
use crate::setup::provider_integration::ProviderIntegration;
use crate::setup::window_setup::ProviderWindowSetup;

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

    fn apply_plugins(
        &self,
        mut builder: tauri::Builder<tauri::Wry>,
        provider: &StorageProvider,
    ) -> tauri::Builder<tauri::Wry> {
        match holochain_plugin(provider.clone()) {
            Ok(plugin) => {
                tracing::info!("[APP BUILDER] Loaded Holochain plugin");
                builder = builder.plugin(plugin);
            }
            Err(e) => {
                tracing::error!("[APP BUILDER] Failed to load Holochain plugin: {}", e);
            }
        }
        builder
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
