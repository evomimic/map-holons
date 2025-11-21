//use crate::config::app_config::APP_ID;
use tauri::{AppHandle, Theme, WebviewUrl, WebviewWindowBuilder};
use async_trait::async_trait;

/// Trait for provider-specific window setup
#[async_trait]
pub trait ProviderWindowSetup: Send + Sync {
    /// Create window with provider-specific integration
    async fn create_window(&self, handle: &AppHandle, app_id: &str) -> anyhow::Result<()>;
}

/// Default window setup (no provider integration)
pub struct DefaultWindowSetup;

#[async_trait]
impl ProviderWindowSetup for DefaultWindowSetup {    
    async fn create_window(&self, handle: &AppHandle, app_id: &str) -> anyhow::Result<()> {
        tracing::debug!("[WINDOW SETUP] Creating default window (no provider integration), appID: {}", app_id );
        
        let _main_window = WebviewWindowBuilder::new(
            handle,
            "main",
            WebviewUrl::App("index.html".into()),
        )
        .theme(Some(Theme::Dark))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create default window: {}", e))?;
        
        Ok(())
    }
}