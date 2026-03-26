use std::sync::RwLock;

use crate::{
    config::storage_manager::StorageManager,
    map_commands as commands, runtime,
    setup::{
        provider_registry::ProviderRegistry,
        receptor_config_registry::ReceptorConfigRegistry,
        setup_manager::SetupManager,
    },
};
use holons_receptor::ReceptorFactory;

pub struct AppBuilder;

impl AppBuilder {
    /// Build and configure the Tauri application
    pub fn build() -> tauri::Builder<tauri::Wry> {
        tracing::debug!("[APP BUILDER] Loading storage configuration.");
        let storage_manager = StorageManager::load_storage_config().unwrap_or_else(|e| {
            tracing::error!("[APP BUILDER] failed: {}", e);
            std::process::exit(1);
        });

        tracing::debug!("[APP BUILDER] Resolving runtime provider selection.");
        let runtime_selection = SetupManager::resolve_runtime_selection(&storage_manager).unwrap_or_else(|e| {
            tracing::error!("[APP BUILDER] failed: {}", e);
            std::process::exit(1);
        });
        if runtime_selection.runtime_provider_keys.is_empty() {
            tracing::error!(
                "[APP BUILDER] failed: at least one storage provider must be enabled."
            );
            std::process::exit(1);
        }
        for warning in &runtime_selection.warnings {
            tracing::warn!("[APP BUILDER] {}", warning);
        }
        tracing::debug!(
            "[APP BUILDER] configuration complete. runtime_providers={:?}, window_provider={:?}",
            runtime_selection.runtime_provider_keys,
            runtime_selection.window_provider_key
        );

        tracing::debug!("[APP BUILDER] Building provider registry.");
        let (runtime_provider_types, registry_warnings) =
            storage_manager.runtime_provider_types(&runtime_selection);
        for warning in registry_warnings {
            tracing::warn!("[APP BUILDER] {}", warning);
        }
        if runtime_provider_types.is_empty() {
            tracing::error!(
                "[APP BUILDER] failed: no runtime provider types resolved from runtime selection."
            );
            std::process::exit(1);
        }

        let registry = ProviderRegistry::with_provider_types(&runtime_provider_types);

        tracing::debug!("[APP BUILDER] Building base Tauri app.");
        let base = tauri::Builder::default()
            .manage(storage_manager.clone())
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

        tracing::debug!("[APP BUILDER] Applying provider plugins.");
        let with_plugins =
            SetupManager::apply_plugins(base, &storage_manager, &runtime_selection, &registry);

        tracing::debug!("[APP BUILDER] Registering setup orchestration.");
        let with_registry = with_plugins.manage(registry);
        let builder = with_registry.setup(SetupManager::setup_handler);
        tracing::info!("[APP BUILDER] App builder done.");
        builder
    }
}
