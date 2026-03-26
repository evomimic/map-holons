use std::sync::Arc;

use crate::{
    config::{
        providers::ProviderRuntimeSelection,
        storage_manager::{StorageManager, StorageProvider},
    },
    runtime,
    setup::{
        provider_registry::ProviderRegistry,
        receptor_config_registry::ReceptorConfigRegistry,
        window_setup::{DefaultWindowSetup, ProviderWindowSetup},
    },
};
use futures::future::join_all;
use holons_receptor::ReceptorFactory;
use tauri::{AppHandle, Listener, Manager};

pub struct SetupManager;

impl SetupManager {
    pub fn resolve_runtime_selection(
        storage_manager: &StorageManager,
    ) -> Result<ProviderRuntimeSelection, String> {
        storage_manager.resolve_runtime_selection()
    }

    /// Apply provider-specific plugins based on runtime provider selection.
    pub fn apply_plugins(
        mut builder: tauri::Builder<tauri::Wry>,
        storage_cfg: &StorageManager,
        runtime_selection: &ProviderRuntimeSelection,
        registry: &ProviderRegistry,
    ) -> tauri::Builder<tauri::Wry> {
        tracing::debug!(
            "[SETUP MANAGER] Loading provider plugins: {:?}",
            runtime_selection.runtime_provider_keys
        );

        builder = builder.plugin(tauri_plugin_fs::init());

        let (runtime_provider_entries, resolution_warnings) =
            storage_cfg.runtime_provider_entries(runtime_selection);
        Self::log_resolution_warnings(&resolution_warnings);

        for (provider_name, provider) in runtime_provider_entries {
            let provider_type = provider.provider_type();
            if let Some(integration) = registry.get(provider_type) {
                builder = integration.apply_plugins(builder, &provider_name, &provider);
            } else {
                tracing::warn!(
                    "[SETUP MANAGER] Unknown provider type '{}' for provider '{}'",
                    provider_type,
                    provider_name
                );
            }
        }
        builder
    }

    /// Setup handler for application initialization.
    pub fn setup_handler(app: &mut tauri::App<tauri::Wry>) -> Result<(), Box<dyn std::error::Error>> {
        let handle = app.handle().clone();
        let storage_cfg = app.state::<StorageManager>().inner().clone();
        let registry = app.state::<ProviderRegistry>();
        let runtime_selection =
            storage_cfg.resolve_runtime_selection().map_err(|e| anyhow::anyhow!(e))?;

        if runtime_selection.runtime_provider_keys.is_empty() {
            return Err(anyhow::anyhow!(
                "at least one storage provider must be enabled"
            )
            .into());
        }

        let (runtime_provider_entries, resolution_warnings) =
            storage_cfg.runtime_provider_entries(&runtime_selection);
        Self::log_resolution_warnings(&resolution_warnings);

        let enabled_providers: Vec<&str> = runtime_provider_entries
            .iter()
            .map(|(_, provider)| provider.provider_type())
            .collect();
        tracing::debug!(
            "[SETUP MANAGER] Setting up providers: {:#?}",
            enabled_providers
        );
        let setup_events = Self::enabled_setup_events(&runtime_provider_entries, registry.inner());

        if setup_events.is_empty() {
            tracing::info!("[SETUP MANAGER] No async provider setup event required.");
            let runtime_selection = runtime_selection.clone();
            tauri::async_runtime::spawn(async move {
                Self::run_complete_setup(&handle, &storage_cfg, &runtime_selection).await;
            });
        } else {
            use std::collections::HashSet;
            use std::sync::Mutex;
            use std::sync::atomic::{AtomicBool, Ordering};

            tracing::debug!(
                "[SETUP MANAGER] Waiting for async setup events from enabled providers: {:?}",
                setup_events
            );

            let pending =
                Arc::new(Mutex::new(setup_events.iter().cloned().collect::<HashSet<String>>()));
            let started = Arc::new(AtomicBool::new(false));

            for event_name in setup_events {
                let pending = Arc::clone(&pending);
                let started = Arc::clone(&started);
                let handle = handle.clone();
                let storage_cfg = storage_cfg.clone();
                let runtime_selection = runtime_selection.clone();
                let event_for_remove = event_name.clone();

                app.handle().listen(event_name, move |_event| {
                    tracing::debug!("[SETUP MANAGER] Received '{}' event.", event_for_remove);

                    let should_start = {
                        let mut guard = pending.lock().expect("pending setup events lock poisoned");
                        guard.remove(&event_for_remove);
                        guard.is_empty()
                    };

                    if should_start && !started.swap(true, Ordering::SeqCst) {
                        let handle_for_setup = handle.clone();
                        let storage_cfg_for_setup = storage_cfg.clone();
                        let runtime_selection_for_setup = runtime_selection.clone();
                        tauri::async_runtime::spawn(async move {
                            Self::run_complete_setup(
                                &handle_for_setup,
                                &storage_cfg_for_setup,
                                &runtime_selection_for_setup,
                            )
                            .await;
                        });
                    }
                });
            }
        }

        Ok(())
    }

    /// Collect unique setup events required by selected runtime providers.
    fn enabled_setup_events(
        runtime_provider_entries: &[(String, StorageProvider)],
        registry: &ProviderRegistry,
    ) -> Vec<String> {
        let mut events = std::collections::BTreeSet::new();

        for (name, provider) in runtime_provider_entries {
            let provider_type = provider.provider_type();
            if let Some(integration) = registry.get(provider_type) {
                if let Some(event) = integration.setup_event() {
                    events.insert(event.to_string());
                }
            } else {
                tracing::warn!(
                    "[SETUP MANAGER] Unknown provider type '{}' for provider '{}'",
                    provider_type,
                    name
                );
            }
        }

        events.into_iter().collect()
    }

    /// Run provider setup, then receptor loading, runtime init, then window creation.
    async fn run_complete_setup(
        handle: &AppHandle,
        storage_cfg: &StorageManager,
        runtime_selection: &ProviderRuntimeSelection,
    ) {
        tracing::debug!("[SETUP MANAGER] Running complete setup.");

        if let Err(e) = Self::apply_setups(handle, storage_cfg, runtime_selection).await {
            tracing::error!("[SETUP MANAGER] Provider setup failed: {}", e);
        }

        if let Err(e) = Self::load_receptor_configs(handle).await {
            tracing::error!("[SETUP MANAGER] Failed to load receptor configs: {}", e);
            return;
        }

        runtime::init_from_state(handle);

        if let Err(e) = Self::create_window(handle, storage_cfg, runtime_selection).await {
            tracing::error!("[SETUP MANAGER] Window creation failed: {}", e);
            return;
        }

        tracing::info!("[SETUP MANAGER] Setup completed successfully.");
    }

    async fn load_receptor_configs(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(registry) = handle.try_state::<ReceptorConfigRegistry>() {
            let configs = registry.all();
            if let Some(factory) = handle.try_state::<ReceptorFactory>() {
                factory.load_from_configs(configs).await?;
                tracing::debug!("[SETUP MANAGER] ReceptorFactory loaded from configs.");
            }
        }
        Ok(())
    }

    /// Run provider-specific setup routines for each selected runtime provider.
    async fn apply_setups(
        handle: &AppHandle,
        storage_cfg: &StorageManager,
        runtime_selection: &ProviderRuntimeSelection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let registry =
            handle.try_state::<ProviderRegistry>().ok_or("Missing ProviderRegistry in state")?;

        let (runtime_provider_entries, resolution_warnings) =
            storage_cfg.runtime_provider_entries(runtime_selection);
        Self::log_resolution_warnings(&resolution_warnings);

        let tasks: Vec<_> = runtime_provider_entries
            .into_iter()
            .filter_map(|(name, provider)| {
                let handle = handle.clone();
                let provider_type = provider.provider_type();
                let integration = match registry.get(provider_type) {
                    Some(integration) => Arc::clone(integration),
                    None => {
                        tracing::warn!(
                            "[SETUP MANAGER] Unknown provider type '{}' for provider '{}'",
                            provider_type,
                            name
                        );
                        return None;
                    }
                };
                tracing::info!("[SETUP MANAGER] Running {} setup for '{}'", provider_type, name);
                Some(tauri::async_runtime::spawn(async move {
                    integration
                        .setup(handle, &name, &provider)
                        .await
                        .map_err(|e| format!("{}/{}: {}", provider_type, name, e))
                }))
            })
            .collect();

        let results = join_all(tasks).await;
        for result in results {
            result
                .map_err(|e| anyhow::anyhow!("Provider setup task panicked: {}", e))?
                .map_err(|e| anyhow::anyhow!(e))?;
        }

        Ok(())
    }

    fn log_resolution_warnings(warnings: &[String]) {
        for warning in warnings {
            tracing::warn!("[SETUP MANAGER] {}", warning);
        }
    }

    /// Create the main application window, using provider-specific window if configured.
    async fn create_window(
        handle: &AppHandle,
        storage_cfg: &StorageManager,
        runtime_selection: &ProviderRuntimeSelection,
    ) -> anyhow::Result<()> {
        if handle.get_webview_window("main").is_some() {
            tracing::info!("[SETUP MANAGER] Main window already exists, skipping creation.");
            return Ok(());
        }

        let registry = handle
            .try_state::<ProviderRegistry>()
            .ok_or_else(|| anyhow::anyhow!("Missing ProviderRegistry in state"))?;
        if let Some(window_provider_key) = &runtime_selection.window_provider_key {
            let Some((name, provider)) = storage_cfg.get_provider_entry(window_provider_key) else {
                tracing::warn!(
                    "[SETUP MANAGER] Runtime selection references missing window provider '{}'; using default",
                    window_provider_key
                );
                let setup = DefaultWindowSetup;
                setup.create_window(handle, "").await?;
                return Ok(());
            };

            let provider_type = provider.provider_type();
            if let Some(integration) = registry.get(provider_type) {
                if integration.supports_window() {
                    tracing::info!(
                        "[SETUP MANAGER] Creating {} window (provider: {})",
                        provider_type,
                        name
                    );
                    integration.create_window(handle, name, provider).await?;
                    return Ok(());
                }
                tracing::info!(
                    "[SETUP MANAGER] Window provider '{}' does not support windows; using default",
                    name
                );
            } else {
                tracing::warn!(
                    "[SETUP MANAGER] Unknown provider type '{}' for window provider '{}'; using default",
                    provider_type,
                    name
                );
            }
            let setup = DefaultWindowSetup;
            setup.create_window(handle, "").await?;
            return Ok(());
        }

        tracing::info!("[SETUP MANAGER] Creating default window");
        let setup = DefaultWindowSetup;
        setup.create_window(handle, "").await?;

        Ok(())
    }
}
