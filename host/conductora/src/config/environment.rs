//! Local environment selection; changing selection never moves or deletes an existing store.
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Default)]
pub struct EnvironmentMismatch(pub RwLock<Option<String>>);

/// Resolve the selected environment. Absence preserves the original installation's path.
pub fn selected_root(base: &Path) -> anyhow::Result<PathBuf> {
    match std::fs::read_to_string(base.join("active-local-environment")) {
        Ok(name) => {
            anyhow::ensure!(valid_name(&name), "Invalid local environment selection");
            let root = base.join("environments").join(name);
            anyhow::ensure!(root.is_dir(), "Selected local environment is missing");
            Ok(root)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(base.join("local-constitution"))
        }
        Err(error) => Err(error.into()),
    }
}

fn valid_name(name: &str) -> bool {
    name.starts_with("environment-") && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Reserve a separate store, then atomically select it for the next process launch.
pub fn create_environment(base: &Path) -> anyhow::Result<PathBuf> {
    let environments = base.join("environments");
    std::fs::create_dir_all(&environments)?;
    let name = format!(
        "environment-{}-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos(),
        std::process::id()
    );
    let root = environments.join(&name);
    std::fs::create_dir(&root)?;
    let pending = root.join("selection");
    std::fs::write(&pending, name)?;
    std::fs::rename(pending, base.join("active-local-environment"))?;
    Ok(root)
}

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
    use super::*;
    #[test]
    fn new_selection_preserves_original_and_previous_environments() {
        let temp = tempdir::TempDir::new("map-environment-selection").unwrap();
        let base = temp.path();
        let original = selected_root(base).unwrap();
        std::fs::create_dir_all(&original).unwrap();
        std::fs::write(original.join("content"), b"retained").unwrap();
        let first = create_environment(base).unwrap();
        assert_eq!(selected_root(base).unwrap(), first);
        std::fs::write(first.join("identity"), b"first").unwrap();
        let second = create_environment(base).unwrap();
        assert_ne!(first, second);
        assert_eq!(selected_root(base).unwrap(), second);
        assert_eq!(std::fs::read(original.join("content")).unwrap(), b"retained");
        assert_eq!(std::fs::read(first.join("identity")).unwrap(), b"first");
        assert!(!second.join("identity").exists());
    }
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
    #[test]
    fn malformed_or_missing_selection_does_not_fall_back_to_another_store() {
        let temp = tempdir::TempDir::new("map-invalid-environment").unwrap();
        for name in ["../elsewhere", "environment-missing", "", "/absolute"] {
            std::fs::write(temp.path().join("active-local-environment"), name).unwrap();
            assert!(selected_root(temp.path()).is_err());
        }
    }
}
