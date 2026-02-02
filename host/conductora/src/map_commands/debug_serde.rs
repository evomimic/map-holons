use holons_client::shared_types::map_request::MapRequestWire;
use tauri::command;

// Add this temporary command to see the raw JSON
#[command]
pub async fn _debug_dance_raw(
    raw_data: serde_json::Value,
) -> Result<String, String> {
    tracing::warn!("[DEBUG] Raw JSON received: {}", raw_data);
    
    // Try to deserialize step by step
    match serde_json::from_value::<MapRequestWire>(raw_data.clone()) {
        Ok(map_request) => {
            tracing::info!("[DEBUG] Successfully parsed MapRequestWire: {:?}", map_request);
            Ok("SUCCESS: MapRequestWire parsed correctly".to_string())
        },
        Err(e) => {
            tracing::error!("[DEBUG] Failed to parse MapRequestWire: {}", e);
            
            // Let's see what fields are missing/wrong
            if let Some(obj) = raw_data.as_object() {
                tracing::error!("[DEBUG] Available fields: {:?}", obj.keys().collect::<Vec<_>>());
            }
            
            Err(format!("Deserialization error: {}", e))
        }
    }
}
