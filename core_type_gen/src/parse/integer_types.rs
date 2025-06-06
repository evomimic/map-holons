use crate::parse::type_header::TypeHeader;
use crate::parse::type_kind_parser::ParseTypeKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct IntegerTypesFile {
    pub type_kind: String,
    pub variants: Vec<IntegerTypeEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IntegerTypeEntry {
    pub variant: String,
    pub header: TypeHeader,
    pub type_name: String,
    pub min_value: String, // stay as string for now due to extreme bounds
    pub max_value: String,
}

pub fn parse_integer_types_yaml(path: &Path) -> Result<IntegerTypesFile, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse {:?}: {}", path, e))
}

impl ParseTypeKind for IntegerTypesFile {
    type TypeSpecItem = IntegerTypeEntry;

    fn type_kind_name() -> &'static str {
        "IntegerTypes"
    }

    fn parse_yaml(path: &Path) -> Result<Self, String> {
        parse_integer_types_yaml(path)
    }

    fn type_spec_items(&self) -> Vec<(String, &Self::TypeSpecItem)> {
        self.variants.iter().map(|v| (v.variant.clone(), v)).collect()
    }
}
