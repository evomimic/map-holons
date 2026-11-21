//! Provider-specific base resolution and shared environment storage.
pub use welcome::environment::*;

use std::path::PathBuf;

/// Packaged local constitution has one fixed Holochain provider.
pub fn packaged_base() -> anyhow::Result<PathBuf> {
    let storage = super::StorageManager::load_storage_config()?;
    let configs: Vec<_> = storage
        .storage_providers
        .values()
        .filter_map(|provider| match provider {
            super::StorageProvider::Holochain(config) if config.enabled => Some(config),
            _ => None,
        })
        .collect();
    anyhow::ensure!(configs.len() == 1, "Expected one packaged Holochain provider");
    crate::setup::providers::holochain::plugins::holochain_base_dir(configs[0])
}

#[cfg(test)]
mod tests {
    #[test]
    fn contextual_setup_errors_retain_compatibility_classification() {
        let mismatch = anyhow::Error::new(
            tauri_plugin_holochain::HolochainRuntimeError::IncompatibleEnvironment(
                "DNA differs".into(),
            ),
        )
        .context("holochain/provider")
        .context("apply_setups failed");
        assert!(mismatch.chain().any(|cause| matches!(
            cause.downcast_ref::<tauri_plugin_holochain::HolochainRuntimeError>(),
            Some(tauri_plugin_holochain::HolochainRuntimeError::IncompatibleEnvironment(_))
        )));
        assert!(anyhow::anyhow!("disk failure")
            .downcast_ref::<tauri_plugin_holochain::HolochainRuntimeError>()
            .is_none());
    }
}
