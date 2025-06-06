use crate::parse::type_header::TypeHeader;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use super::type_kind_parser::ParseTypeKind;

impl ParseTypeKind for HolonTypesFile {
    type TypeSpecItem = HolonTypeEntry;

    fn type_kind_name() -> &'static str {
        "HolonTypes"
    }

    fn parse_yaml(path: &Path) -> Result<Self, String> {
        parse_holon_types_yaml(path)
    }

    fn type_spec_items(&self) -> Vec<(String, &Self::TypeSpecItem)> {
        self.variants.iter().map(|v| (v.variant.clone(), v)).collect()
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct HolonTypesFile {
    pub type_kind: String,
    pub variants: Vec<HolonTypeEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HolonTypeEntry {
    pub variant: String,
    pub header: TypeHeader,
    pub properties: Vec<String>,
    pub key_properties: Vec<String>,
    pub source_for: Vec<String>,
}

pub fn parse_holon_types_yaml(path: &Path) -> Result<HolonTypesFile, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse {:?}: {}", path, e))
}
