use holons_client::shared_types::map_request::MapRequest;
use tauri::command;

// Add this temporary command to see the raw JSON
#[command]
pub async fn _debug_dance_raw(
    raw_data: serde_json::Value,
) -> Result<String, String> {
    tracing::warn!("[DEBUG] Raw JSON received: {}", raw_data);
    
    // Try to deserialize step by step
    match serde_json::from_value::<MapRequest>(raw_data.clone()) {
        Ok(map_request) => {
            tracing::info!("[DEBUG] Successfully parsed MapRequest: {:?}", map_request);
            Ok("SUCCESS: MapRequest parsed correctly".to_string())
        },
        Err(e) => {
            tracing::error!("[DEBUG] Failed to parse MapRequest: {}", e);
            
            // Let's see what fields are missing/wrong
            if let Some(obj) = raw_data.as_object() {
                tracing::error!("[DEBUG] Available fields: {:?}", obj.keys().collect::<Vec<_>>());
            }
            
            Err(format!("Deserialization error: {}", e))
        }
    }
}