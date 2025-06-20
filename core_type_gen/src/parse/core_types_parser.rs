use crate::parse::core_entry_types::CoreTypeEntry;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CoreTypesInput {
    Wrapped { types: Vec<CoreTypeEntry> },
    Flat(Vec<CoreTypeEntry>),
}

/// Parses a YAML file defining core type entries in either of two supported formats:
///
/// ### ✅ Supported Formats:
///
/// 1. **Wrapped Format** (recommended):
/// ```yaml
/// types:
///   - type_kind: Holon
///     type_name: HolonType
///     ...
/// ```
///
/// 2. **Flat Format** (also supported):
/// ```yaml
/// - type_kind: Holon
///   type_name: HolonType
///   ...
/// ```
pub fn parse_core_type_entries(path: &Path) -> Result<Vec<CoreTypeEntry>, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

    match serde_yaml::from_str::<CoreTypesInput>(&contents) {
        Ok(CoreTypesInput::Wrapped { types }) => Ok(types),
        Ok(CoreTypesInput::Flat(list)) => Ok(list),
        Err(e) => Err(format!("Failed to parse {:?}: {}", path, e)),
    }
}
