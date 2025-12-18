
use holons_client::shared_types::{map_request::MapRequest};
use tauri::{command };

#[command]
pub(crate) async fn serde_test(
    request_json: String,
    receptor_id: String,
) -> Result<String, String> {

    tracing::info!("[TEST] Received JSON string: {}", receptor_id);

    // Try to parse the JSON into a generic Value first for debugging
    match serde_json::from_str::<serde_json::Value>(&request_json) {
        Ok(json_value) => {
            tracing::debug!("[TEST] JSON structure: {:#?}", json_value);
            tracing::debug!("[DEBUG] Available fields: {:?}", json_value.as_object().map(|o| o.keys().collect::<Vec<_>>()));
        }
        Err(e) => {
            tracing::error!("[TEST] Invalid JSON: {}", e);
        }
    }

    //HERE add the type you want to check against in the from_str<>
    match serde_json::from_str::<MapRequest>(&request_json) {
        Ok(request) => {
            tracing::info!("[TEST] Successfully parsed: {:?}", request);
            Ok("Parsed successfully".to_string())
        }
        Err(e) => {
            // Extract detailed location info
            let line = e.line();
            let column = e.column();
            let classification = format!("{:?}", e.classify());

            // Extract the character context around the error
            let error_snippet = if column > 0 && column <= request_json.len() {
                let start = column.saturating_sub(50);
                let end = (column + 50).min(request_json.len());
                let snippet = &request_json[start..end];

                // Calculate relative position of error in snippet
                let error_pos = column - start - 1;
                let pointer = " ".repeat(error_pos) + "^";

                format!("...{}...\n{}", snippet, pointer)
            } else {
                "Unable to extract context".to_string()
            };

            // Try to determine the JSON path from the error message
            let error_string = e.to_string();
            let path_hint = if error_string.contains("missing field") {
                // Extract field name from error message
                let field_name = error_string
                    .split("missing field `")
                    .nth(1)
                    .and_then(|s| s.split('`').next())
                    .unwrap_or("unknown");
                format!("\n🔍 Missing field: `{}`", field_name)
            } else if error_string.contains("invalid type") {
                format!(
                    "\n🔍 Type mismatch - check field types match between TypeScript and Rust"
                )
            } else {
                String::new()
            };

            let error_msg = format!(
                "Parse error at position {} (line {}, column {}):\n\
                 📋 Error: {}\n\
                 🏷️  Type: {}{}\n\
                 📍 Context around column {}:\n{}",
                column, line, column, e, classification, path_hint, column, error_snippet
            );

            tracing::error!("[TEST] {}", error_msg);

            // Return a more concise error to the frontend
            let short_error = format!("Parse error: {} at column {}{}", e, column, path_hint);

            Err(short_error)
        }
    }
    
}