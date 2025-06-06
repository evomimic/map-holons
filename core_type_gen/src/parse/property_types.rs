use crate::parse::type_header::TypeHeader;
use crate::parse::type_kind_parser::ParseTypeKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct PropertyTypesFile {
    pub type_kind: String,
    pub variants: Vec<PropertyTypeEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PropertyTypeEntry {
    pub variant: String,
    pub header: TypeHeader,
    pub property_name: String,
    pub value_type_name: String,
}

pub fn parse_property_types_yaml(path: &Path) -> Result<PropertyTypesFile, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse {:?}: {}", path, e))
}

impl ParseTypeKind for PropertyTypesFile {
    type TypeSpecItem = PropertyTypeEntry;

    fn type_kind_name() -> &'static str {
        "PropertyTypes"
    }

    fn parse_yaml(path: &Path) -> Result<Self, String> {
        parse_property_types_yaml(path)
    }

    fn type_spec_items(&self) -> Vec<(String, &Self::TypeSpecItem)> {
        self.variants.iter().map(|v| (v.variant.clone(), v)).collect()
    }
}
