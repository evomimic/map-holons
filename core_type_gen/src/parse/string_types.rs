use crate::parse::type_header::TypeHeader;
use crate::parse::type_kind_parser::ParseTypeKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct StringTypesFile {
    pub type_kind: String,
    pub variants: Vec<StringTypeEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StringTypeEntry {
    pub variant: String,
    pub header: TypeHeader,
    pub type_name: String,
    pub min_length: u32,
    pub max_length: u32,
}

pub fn parse_string_types_yaml(path: &Path) -> Result<StringTypesFile, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse {:?}: {}", path, e))
}

impl ParseTypeKind for StringTypesFile {
    type TypeSpecItem = StringTypeEntry;

    fn type_kind_name() -> &'static str {
        "StringTypes"
    }

    fn parse_yaml(path: &Path) -> Result<Self, String> {
        parse_string_types_yaml(path)
    }

    fn type_spec_items(&self) -> Vec<(String, &Self::TypeSpecItem)> {
        self.variants.iter().map(|v| (v.variant.clone(), v)).collect()
    }
}
