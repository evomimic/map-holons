use crate::parse::core_entry_types::CoreTypeEntry;
use crate::parse::schema_definition::SchemaBlock;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct CombinedTypesFile {
    pub schema: SchemaBlock,
    pub types: Vec<CoreTypeEntry>,
}

/// Write a single JSON file representing the entire schema.
pub fn generate_consolidated_core_schema(
    schema_block: SchemaBlock,
    entries: Vec<CoreTypeEntry>,
    output_path: &Path,
) -> Result<(), String> {
    let combined = CombinedTypesFile { schema: schema_block, types: entries };

    let json =
        serde_json::to_string_pretty(&combined).map_err(|e| format!("Serialization error: {e}"))?;

    fs::create_dir_all(output_path.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|e| format!("Failed to create output dir: {e}"))?;

    fs::write(output_path, json).map_err(|e| format!("Failed to write output JSON: {e}"))?;

    println!("✅ Wrote consolidated schema to {:?}", output_path);
    Ok(())
}
