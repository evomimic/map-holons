use async_trait::async_trait;
use tauri::AppHandle;

use crate::config::StorageProvider;
use crate::setup::provider_integration::ProviderIntegration;

pub struct IpfsProvider;

impl IpfsProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProviderIntegration for IpfsProvider {
    fn provider_type(&self) -> &'static str {
        "ipfs"
    }

    fn apply_plugins(
        &self,
        builder: tauri::Builder<tauri::Wry>,
        _provider_key: &str,
        _provider: &StorageProvider,
    ) -> tauri::Builder<tauri::Wry> {
        builder
    }

    async fn setup(
        &self,
        _handle: AppHandle,
        _name: &str,
        _provider: &StorageProvider,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
