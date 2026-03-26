use async_trait::async_trait;
use tauri::AppHandle;

use crate::config::StorageProvider;
use crate::setup::provider_integration::ProviderIntegration;

use super::setup::LocalSetup;

pub struct LocalProvider;

impl LocalProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProviderIntegration for LocalProvider {
    fn provider_type(&self) -> &'static str {
        "local"
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
        handle: AppHandle,
        name: &str,
        provider: &StorageProvider,
    ) -> anyhow::Result<()> {
        LocalSetup::setup(handle, name, provider).await
    }
}
