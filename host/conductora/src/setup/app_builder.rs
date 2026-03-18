use std::sync::{Arc, RwLock};

use crate::{
    map_commands as commands,
    runtime,
    config::storage_config::StorageConfig,
    setup::{
        provider_registry::ProviderRegistry,
        receptor_config_registry::ReceptorConfigRegistry,
        window_setup::{DefaultWindowSetup, ProviderWindowSetup},
    },
};
use futures::future::join_all;
use holons_receptor::ReceptorFactory;
use tauri::{AppHandle, Manager, Listener};

pub struct AppBuilder;

impl AppBuilder {
    /// Build and configure the Tauri application
    pub fn build() -> tauri::Builder<tauri::Wry> {
        tracing::debug!("[APP BUILDER] Setting up Tauri application.");
        // Load storage config — abort immediately if unavailable
        let storage_cfg = StorageConfig::load_storage_config().unwrap_or_else(|e| {
            tracing::error!("[APP BUILDER] Cannot start: {}", e);
            std::process::exit(1);
        });
        if storage_cfg.get_enabled_providers().is_empty() {
            tracing::error!(
                "[APP BUILDER] Cannot start: at least one storage provider must be enabled."
            );
            std::process::exit(1);
        }
        let registry = ProviderRegistry::with_defaults();
        // Base builder without setup
        let base = tauri::Builder::default()
            .manage(storage_cfg.clone())
            .manage::<runtime::RuntimeInitiatorState>(RwLock::new(None))
            .manage(ReceptorFactory::new())
            .manage(ReceptorConfigRegistry::new())
            .manage::<runtime::RuntimeState>(RwLock::new(None))
            .invoke_handler(tauri::generate_handler![
                commands::root_space,
                //commands::load_holons,
                commands::serde_test,
                commands::map_request,
                commands::all_spaces,
                commands::is_service_ready,
                runtime::dispatch_map_command::dispatch_map_command,
            ]);
        // First apply provider-specific plugins
        let with_plugins = Self::apply_plugins(base, &storage_cfg, &registry);
        let with_registry = with_plugins.manage(registry);
        // Then register the common setup handler
        with_registry.setup(Self::setup_handler)
    }

    /// Apply provider-specific plugins based on the storage configuration
    fn apply_plugins(
        mut builder: tauri::Builder<tauri::Wry>,
        storage_cfg: &StorageConfig,
        registry: &ProviderRegistry,
    ) -> tauri::Builder<tauri::Wry> {
        tracing::debug!("[APP BUILDER] Loading provider plugins: {:?}", storage_cfg.get_enabled_providers());

        builder = builder.plugin(tauri_plugin_fs::init());

        for (name, provider) in storage_cfg.get_enabled_providers() {
            let provider_type = provider.provider_type();
            if let Some(integration) = registry.get(provider_type) {
                builder = integration.apply_plugins(builder, provider);
            } else {
                tracing::warn!(
                    "[APP BUILDER] Unknown provider type '{}' for provider '{}'",
                    provider_type,
                    name
                );
            }
        }
        builder
    }

    /// Setup handler for application initialization
    fn setup_handler(app: &mut tauri::App<tauri::Wry>) -> Result<(), Box<dyn std::error::Error>> {

        let handle = app.handle().clone();
        let storage_cfg = app.state::<StorageConfig>().inner().clone();
        let registry = app.state::<ProviderRegistry>();
        let enabled_providers: Vec<&str> = storage_cfg
            .get_enabled_providers()
            .iter()
            .map(|(_, p)| p.provider_type())
            .collect();
        tracing::debug!("[APP BUILDER] setting up providers: {:#?}", enabled_providers);

        let default_name = storage_cfg.default_storage.clone();
        let setup_event = match storage_cfg.get_provider(&default_name) {
            Some(provider) => {
                if !provider.is_enabled() {
                    tracing::warn!(
                        "[APP BUILDER] Default provider '{}' is disabled; skipping setup gating.",
                        default_name
                    );
                    None
                } else {
                    match registry.get(provider.provider_type()) {
                        Some(integration) => integration.setup_event(),
                        None => {
                            tracing::warn!(
                                "[APP BUILDER] No integration found for default provider type '{}'; skipping setup gating.",
                                provider.provider_type()
                            );
                            None
                        }
                    }
                }
            }
            None => {
                tracing::warn!(
                    "[APP BUILDER] Default provider '{}' not found; skipping setup gating.",
                    default_name
                );
                None
            }
        };

        if let Some(event) = setup_event {
            tracing::debug!(
                "[APP BUILDER] Setup event '{}' detected for default provider, waiting.",
                event
            );
            app.handle().listen(event, move |_event| {
                tracing::debug!("[APP BUILDER] Received '{}' event.", event);
                let handle = handle.clone();
                let storage_cfg = storage_cfg.clone(); // ← Clone for the closure
                tauri::async_runtime::spawn(async move {
                    Self::run_complete_setup(&handle, &storage_cfg).await;
                });
            });
        } else {
            tracing::info!("[APP BUILDER] No async provider setup required.");
            tauri::async_runtime::spawn(async move {
                Self::run_complete_setup(&handle, &storage_cfg).await;
            });
        }

        Ok(())
    }

    /// Run the complete setup: provider setup → load receptors → create window
    async fn run_complete_setup(handle: &AppHandle, storage_cfg: &StorageConfig) {
        tracing::debug!("[APP BUILDER] Running complete setup.");

        // Run provider-specific setup routines
        if let Err(e) = Self::apply_setups(handle).await {
            tracing::error!("[APP BUILDER] Provider setup failed: {}", e);
        }

        // Load receptor configs into factory
        if let Err(e) = Self::load_receptor_configs(handle).await {
            tracing::error!("[APP BUILDER] Failed to load receptor configs: {}", e);
            return;
        }

        // Construct the MAP Commands Runtime (if a runtime initiator is available)
        runtime::init_from_state(handle);

        // Create main window
        if let Err(e) = Self::create_window(handle, storage_cfg).await {
            tracing::error!("[APP BUILDER] Window creation failed: {}", e);
            return; // ← This is fine since function returns ()
        }

        tracing::info!("[APP BUILDER] Setup completed successfully.");
    }

    /// Load receptor configs from registry into factory
    async fn load_receptor_configs(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(registry) = handle.try_state::<ReceptorConfigRegistry>() {
            let configs = registry.all();
            if let Some(factory) = handle.try_state::<ReceptorFactory>() {
                factory.load_from_configs(configs).await?;
                tracing::debug!("[APP BUILDER] ReceptorFactory loaded from configs.");
            }
        }
        Ok(())
    }

    /// Run provider-specific setup routines for each enabled provider
    async fn apply_setups(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        let storage_cfg = handle.try_state::<StorageConfig>()
            .ok_or("Missing StorageConfig in state")?;
        let registry = handle
            .try_state::<ProviderRegistry>()
            .ok_or("Missing ProviderRegistry in state")?;

        let tasks: Vec<_> = storage_cfg
            .get_enabled_providers()
            .into_iter()
            .filter_map(|(name, provider)| {
                let handle = handle.clone();
                let name = name.clone();
                let provider = provider.clone();
                let provider_type = provider.provider_type();
                let integration = match registry.get(provider_type) {
                    Some(integration) => Arc::clone(integration),
                    None => {
                        tracing::warn!(
                            "[APP BUILDER] Unknown provider type '{}' for provider '{}'",
                            provider_type,
                            name
                        );
                        return None;
                    }
                };
                tracing::info!(
                    "[APP BUILDER] Running {} setup for '{}'",
                    provider_type,
                    name
                );
                Some(tauri::async_runtime::spawn(async move {
                    integration
                        .setup(handle, &name, &provider)
                        .await
                        .map_err(|e| format!("{}/{}: {}", provider_type, name, e))
                }))
            })
            .collect();

        // All tasks are now running concurrently; await each and surface any error
        let results = join_all(tasks).await;
        for result in results {
            // Outer Err = task panicked; inner Err = setup failed
            result
                .map_err(|e| anyhow::anyhow!("Provider setup task panicked: {}", e))?
                .map_err(|e| anyhow::anyhow!(e))?;
        }

        Ok(())
    }

    /// Create the main application window, using provider-specific window if configured
    async fn create_window(handle: &AppHandle, storage_cfg: &StorageConfig) -> anyhow::Result<()> {
        // Check if window already exists
        if handle.get_webview_window("main").is_some() {
            tracing::info!("[APP BUILDER] Main window already exists, skipping creation.");
            return Ok(());
        }

        let registry = handle
            .try_state::<ProviderRegistry>()
            .ok_or_else(|| anyhow::anyhow!("Missing ProviderRegistry in state"))?;
        let window_provider = storage_cfg
            .resolve_window_provider()
            .map_err(|e| anyhow::anyhow!(e))?;
        match window_provider {
            Some((name, provider)) => {
                let provider_type = provider.provider_type();
                if let Some(integration) = registry.get(provider_type) {
                    if integration.supports_window() {
                        tracing::info!(
                            "[APP BUILDER] Creating {} window (provider: {})",
                            provider_type,
                            name
                        );
                        integration.create_window(handle, name, provider).await?;
                        return Ok(());
                    }
                    tracing::info!(
                        "[APP BUILDER] Window provider '{}' does not support windows; using default",
                        name
                    );
                } else {
                    tracing::warn!(
                        "[APP BUILDER] Unknown provider type '{}' for window provider '{}'; using default",
                        provider_type,
                        name
                    );
                }
                let setup = DefaultWindowSetup;
                setup.create_window(handle, "").await?;
            }
            None => {
                tracing::info!("[APP BUILDER] Creating default window");
                let setup = DefaultWindowSetup;
                setup.create_window(handle, "").await?;
            }
        }

        Ok(())
    }

}
