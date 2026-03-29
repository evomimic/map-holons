use std::sync::{Arc, Mutex};

use crate::{
    config::{
        providers::ProviderRuntimeSelection,
        storage_manager::StorageManager,
    },
    setup::{
        provider_registry::ProviderRegistry,
        window_setup::{DefaultWindowSetup, ProviderWindowSetup},
    },
};
use futures::channel::oneshot;
use futures::future::select_all;
use tauri::{AppHandle, Listener, Manager};

pub struct SetupManager;

type ReadySender = Arc<Mutex<Option<oneshot::Sender<anyhow::Result<()>>>>>;

impl SetupManager {
    fn resolve_ready(ready_sender: &ReadySender, result: anyhow::Result<()>) {
        let sender = ready_sender
            .lock()
            .expect("ready sender lock poisoned")
            .take();

        if let Some(sender) = sender {
            if sender.send(result).is_err() {
                tracing::warn!("[SETUP MANAGER] Readiness receiver dropped before completion.");
            }
        }
    }

    async fn wait_for_provider_ready(
        handle: &AppHandle,
        provider_name: &str,
        integration: &Arc<dyn crate::setup::provider_integration::ProviderIntegration>,
    ) -> anyhow::Result<()> {
        let Some(success_event) = integration.setup_event() else {
            return Ok(());
        };

        if integration.is_ready(handle) {
            return Ok(());
        }

        tracing::debug!(
            "[SETUP MANAGER] Waiting for provider '{}' readiness event '{}'.",
            provider_name,
            success_event
        );

        let (tx, rx) = oneshot::channel();
        let ready_sender = Arc::new(Mutex::new(Some(tx)));
        let provider_name = provider_name.to_string();
        let provider_name_for_wait_error = provider_name.clone();

        let success_event_name = success_event.to_string();
        let success_sender = Arc::clone(&ready_sender);
        let success_id = handle.once(success_event_name.clone(), move |_event| {
            tracing::debug!(
                "[SETUP MANAGER] Received '{}' readiness event.",
                success_event_name
            );
            Self::resolve_ready(&success_sender, Ok(()));
        });

        let failure_id = integration.setup_failed_event().map(|failure_event| {
            let failure_event_name = failure_event.to_string();
            let failure_sender = Arc::clone(&ready_sender);
            let provider_name = provider_name.clone();
            handle.once(failure_event_name.clone(), move |_event| {
                tracing::error!(
                    "[SETUP MANAGER] Received '{}' failure event.",
                    failure_event_name
                );
                Self::resolve_ready(
                    &failure_sender,
                    Err(anyhow::anyhow!(
                        "Provider '{}' emitted startup failure event '{}'",
                        provider_name,
                        failure_event_name
                    )),
                );
            })
        });

        if integration.is_ready(handle) {
            handle.unlisten(success_id);
            if let Some(failure_id) = failure_id {
                handle.unlisten(failure_id);
            }
            Self::resolve_ready(&ready_sender, Ok(()));
        }

        let result = match rx.await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "provider '{}' readiness channel closed unexpectedly",
                provider_name_for_wait_error
            )),
        };

        handle.unlisten(success_id);
        if let Some(failure_id) = failure_id {
            handle.unlisten(failure_id);
        }

        result
    }

    /// Run provider-specific setup routines for each selected runtime provider.
    pub async fn apply_setups(
        handle: &AppHandle,
        storage_cfg: &StorageManager,
        runtime_selection: &ProviderRuntimeSelection,
    ) -> anyhow::Result<()> {
        let registry = handle
            .try_state::<ProviderRegistry>()
            .ok_or_else(|| anyhow::anyhow!("Missing ProviderRegistry in state"))?;

        let (runtime_provider_entries, _) = storage_cfg.runtime_provider_entries(runtime_selection);

        let mut tasks: Vec<_> = runtime_provider_entries
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
                    Self::wait_for_provider_ready(&handle, &name, &integration).await?;
                    integration
                        .setup(handle, &name, &provider)
                        .await
                        .map_err(|e| anyhow::anyhow!("{}/{}: {}", provider_type, name, e))
                }))
            })
            .collect();

        while !tasks.is_empty() {
            let (result, _index, remaining) = select_all(tasks).await;
            tasks = remaining;

            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    for task in &tasks {
                        task.abort();
                    }
                    return Err(anyhow::anyhow!(e));
                }
                Err(e) => {
                    for task in &tasks {
                        task.abort();
                    }
                    return Err(anyhow::anyhow!("Provider setup task panicked: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Create the main application window, using provider-specific window if configured.
    pub async fn create_window(
        handle: &AppHandle,
        storage_cfg: &StorageManager,
        runtime_selection: &ProviderRuntimeSelection,
    ) -> anyhow::Result<()> {
        if handle.get_webview_window("main").is_some() {
            tracing::debug!("[SETUP MANAGER] Main window already exists, skipping creation.");
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
