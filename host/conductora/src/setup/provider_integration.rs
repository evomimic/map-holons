use async_trait::async_trait;
use tauri::AppHandle;

use crate::config::StorageProvider;

/// Provider-specific integration points (plugins, setup, window).
#[async_trait]
pub trait ProviderIntegration: Send + Sync {
    fn provider_type(&self) -> &'static str;

    /// Optional event that gates setup completion.
    fn setup_event(&self) -> Option<&'static str> {
        None
    }

    /// Optional event that indicates startup failure before provider setup can run.
    fn setup_failed_event(&self) -> Option<&'static str> {
        None
    }

    /// Whether this provider is already ready to run setup.
    fn is_ready(&self, _handle: &AppHandle) -> bool {
        self.setup_event().is_none()
    }

    /// Apply provider-specific plugins to the Tauri builder.
    fn apply_plugins(
        &self,
        builder: tauri::Builder<tauri::Wry>,
        provider_key: &str,
        provider: &StorageProvider,
    ) -> anyhow::Result<tauri::Builder<tauri::Wry>>;

    /// Provider-specific setup routine.
    async fn setup(
        &self,
        handle: AppHandle,
        name: &str,
        provider: &StorageProvider,
    ) -> anyhow::Result<()>;

    /// Whether this provider supports a custom window setup.
    fn supports_window(&self) -> bool {
        false
    }

    /// Provider-specific window creation (if supported).
    async fn create_window(
        &self,
        _handle: &AppHandle,
        _provider_name: &str,
        _provider: &StorageProvider,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
