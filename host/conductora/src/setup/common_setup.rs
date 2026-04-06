use crate::{
    config::providers::ProviderConfig,
    setup::receptor_config_registry::ReceptorConfigRegistry,
};
use client_shared_types::base_receptor::BaseReceptor;
use tauri::{AppHandle, Manager};


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

/// Register a built receptor config into the application state
pub async fn register_receptor(
    handle: &AppHandle,
    receptor_cfg: BaseReceptor,
) -> anyhow::Result<()> {
    // Get the registry from app state and register the new config
    let registry = handle.state::<ReceptorConfigRegistry>();
    registry.register(receptor_cfg);
    Ok(())
}
