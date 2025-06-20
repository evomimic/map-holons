//! Core Type Generator Entry Point
//!
//! This binary parses and processes the MAP Core Type System definitions, producing:
//!
//! 1. A **consolidated JSON file** (`core_schema.json`) containing all type definitions listed
//!    in `schema.yml`, across all `type_kind`s.
//!
//! 2. A **set of Rust enums**, one per `type_kind` (e.g., `CoreHolonTypeName`), each representing
//!    the canonical variant names for use in generated code.

mod generate;
mod parse;
mod templates;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::generate::core_type_generator::generate_consolidated_core_schema;
use crate::generate::enum_template::generate_enum_from_template;
use crate::parse::core_entry_types::CoreTypeEntry;
use crate::parse::core_types_parser::parse_core_type_entries;
use crate::parse::schema_definition::SchemaDefinitionFile;

fn load_schema_def(path: &Path) -> Result<SchemaDefinitionFile, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read schema file: {e}"))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse schema file: {e}"))
}

fn main() -> Result<(), String> {
    let schema_path = Path::new("core_type_gen/core_type_defs/test_schema.yml");
    let output_json = Path::new("core_type_gen/generated_specs/core_schema.json");

    let schema_def = load_schema_def(schema_path)?;
    let base_dir = schema_path.parent().unwrap_or_else(|| Path::new("."));

    let mut type_entries: Vec<CoreTypeEntry> = Vec::new();
    let mut kind_to_names: HashMap<String, Vec<String>> = HashMap::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for file_name in &schema_def.type_files {
        let file_path = base_dir.join(file_name);
        let entries = parse_core_type_entries(&file_path)?;

        for entry in entries {
            let name = entry.get_variant_name().to_string();

            if !seen_names.insert(name.clone()) {
                return Err(format!(
                    "❌ Duplicate type_name '{}' found in file {:?}",
                    name, file_path
                ));
            }

            let kind = entry.get_type_kind();
            kind_to_names.entry(kind).or_default().push(name);
            type_entries.push(entry);
        }
    }

    generate_consolidated_core_schema(schema_def.schema, type_entries, output_json)?;

    for (kind, names) in kind_to_names {
        let enum_name = format!("Core{}TypeName", kind);
        let out_path = format!(
            "crates/type_system/type_names/src/generated/core_{}_type_name.rs",
            kind.to_lowercase()
        );
        generate_enum_from_template(&enum_name, &names, Path::new(&out_path))?;
    }

    Ok(())
}
