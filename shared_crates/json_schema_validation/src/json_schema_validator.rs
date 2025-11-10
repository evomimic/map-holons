use integrity_core_types::validation_error::ValidationError;
use jsonschema::validator_for;
use serde_json::Value;
use std::{fs::File, io::BufReader, path::Path};

/// Validate `json_path` against the JSON Schema at `schema_path`.
/// - Uses draft autodetection + `$ref` support
/// - Handles large files via `BufReader`
/// - Returns all errors joined by newline
pub fn validate_json_against_schema(
    schema_path: &Path,
    json_path: &Path,
) -> Result<(), ValidationError> {
    // Load schema document
    let schema_file = File::open(schema_path)
        .map_err(|e| ValidationError::JsonSchemaError(format!("Error opening schema file: {e}")))?;
    let schema: Value = serde_json::from_reader(BufReader::new(schema_file))
        .map_err(|e| ValidationError::JsonSchemaError(format!("Invalid schema JSON: {e}")))?;

    // Build validator (auto-detects draft + handles `$ref`)
    let validator = validator_for(&schema)
        .map_err(|e| ValidationError::JsonSchemaError(format!("Schema compile error: {e}")))?;

    // Load JSON instance
    let data_file = File::open(json_path)
        .map_err(|e| ValidationError::JsonSchemaError(format!("Error opening JSON file: {e}")))?;
    let instance: Value = serde_json::from_reader(BufReader::new(data_file))
        .map_err(|e| ValidationError::JsonSchemaError(format!("Invalid input JSON: {e}")))?;

    // Collect *all* validation errors
    let errors: Vec<String> = validator.iter_errors(&instance).map(|e| e.to_string()).collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationError::JsonSchemaError(errors.join("\n")))
    }
}
