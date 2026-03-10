use crate::config::ProviderConfig;
use core_types::HolonError;
use holons_recovery::TransactionRecoveryStore;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Create a snapshot recovery store for any provider config type that implements `ProviderConfig`.
///
/// - Returns `Ok(None)` if `snapshot_recovery` is not enabled in the config.
/// - Returns `Ok(Some(store))` if a snapshot store was successfully created.
/// - Returns `Err` if the app data directory cannot be resolved or the store cannot be created.
///
/// The database is placed at: `{app_data_dir}/storage/{name}/snapshots.db`
///
/// Blocking I/O (dir creation + SQLite open) is offloaded via `spawn_blocking`.
pub async fn create_snapshot_store<C: ProviderConfig>(
    handle: &AppHandle,
    config: &C,
    name: &str,
) -> Result<Option<Arc<TransactionRecoveryStore>>, HolonError> {
    if !config.snapshot_recovery() {
        tracing::debug!(
            "[SNAPSHOT] Skipping snapshot store for '{}' (snapshot_recovery not enabled)",
            name
        );
        return Ok(None);
    }

    // Path resolution is non-blocking — do it on the async thread
    let app_data_dir = handle
        .path()
        .app_data_dir()
        .map_err(|e| HolonError::Misc(format!("Failed to resolve app data dir: {}", e)))?;

    let snapshot_dir = app_data_dir.join("storage").join(name);
    let db_path = snapshot_dir.join("snapshots.db");
    tracing::info!("[SNAPSHOT] Creating snapshot store at: {:?}", db_path);

    // Offload blocking filesystem + SQLite open to a dedicated thread
    let store =
        tokio::task::spawn_blocking(move || -> Result<TransactionRecoveryStore, HolonError> {
            std::fs::create_dir_all(&snapshot_dir).map_err(|e| {
                HolonError::Misc(format!("Failed to create snapshot dir {:?}: {}", snapshot_dir, e))
            })?;
            TransactionRecoveryStore::new(&db_path)
        })
        .await
        .map_err(|e| HolonError::Misc(format!("spawn_blocking panicked: {}", e)))??;

    Ok(Some(Arc::new(store)))
}

pub fn serialize_props<C: ProviderConfig>(config: &C) -> std::collections::HashMap<String, String> {
    match serde_json::to_value(config) {
        Ok(serde_json::Value::Object(map)) => map
            .into_iter()
            .map(|(k, v)| {
                let value_str = match v {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => String::new(),
                    _ => v.to_string(),
                };
                (k, value_str)
            })
            .collect::<std::collections::HashMap<String, String>>(),
        _ => std::collections::HashMap::new(),
    }
}
