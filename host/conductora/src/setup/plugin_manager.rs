use crate::{
    config::{providers::ProviderRuntimeSelection, storage_manager::StorageManager},
    setup::provider_registry::ProviderRegistry,
};

pub struct PluginManager;

impl PluginManager {
    /// Apply provider-specific plugins based on runtime provider selection.
    pub fn apply_plugins(
        mut builder: tauri::Builder<tauri::Wry>,
        storage_cfg: &StorageManager,
        runtime_selection: &ProviderRuntimeSelection,
        registry: &ProviderRegistry,
    ) -> anyhow::Result<tauri::Builder<tauri::Wry>> {
        tracing::debug!(
            "[PLUGIN MANAGER] Loading provider plugins: {:?}",
            runtime_selection.runtime_provider_keys
        );

        builder = builder.plugin(tauri_plugin_fs::init());

        let (runtime_provider_entries, _) = storage_cfg.runtime_provider_entries(runtime_selection);

        for (provider_name, provider) in runtime_provider_entries {
            let provider_type = provider.provider_type();
            if let Some(integration) = registry.get(provider_type) {
                builder = integration.apply_plugins(builder, &provider_name, &provider)?;
            } else {
                tracing::warn!(
                    "[PLUGIN MANAGER] Unknown provider type '{}' for provider '{}'",
                    provider_type,
                    provider_name
                );
            }
        }

        Ok(builder)
    }
}
