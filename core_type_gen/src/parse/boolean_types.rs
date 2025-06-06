use crate::parse::type_header::TypeHeader;
use crate::parse::type_kind_parser::ParseTypeKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct BooleanTypesFile {
    pub type_kind: String,
    pub variants: Vec<BooleanTypeEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BooleanTypeEntry {
    pub variant: String,
    pub type_name: String,
    pub header: TypeHeader,
}

pub fn parse_boolean_types_yaml(path: &Path) -> Result<BooleanTypesFile, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse {:?}: {}", path, e))
}

impl ParseTypeKind for BooleanTypesFile {
    type TypeSpecItem = BooleanTypeEntry;

    fn type_kind_name() -> &'static str {
        "BooleanTypes"
    }

    fn parse_yaml(path: &Path) -> Result<Self, String> {
        parse_boolean_types_yaml(path)
    }

    fn type_spec_items(&self) -> Vec<(String, &Self::TypeSpecItem)> {
        self.variants.iter().map(|v| (v.variant.clone(), v)).collect()
    }
}
