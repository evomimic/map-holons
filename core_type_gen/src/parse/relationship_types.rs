use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct RelationshipTypesFile {
    pub type_kind: String,
    pub variants: Vec<RelationshipTypeEntry>,
}

#[derive(Debug, Deserialize)]
pub struct RelationshipTypeEntry {
    pub variant: String,
    pub header: RelationshipTypeHeader,
    pub source_type: String,
    pub target_type: String,
    pub deletion_semantic: Option<String>,
    pub inverse_name: Option<String>,
    pub inverse_of: Option<String>,
    pub key_properties: Vec<String>,
    pub descriptor_properties: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RelationshipTypeHeader {
    pub descriptor_name: String,
    pub description: String,
    pub label: String,
    pub is_dependent: bool,
    pub is_value_type: bool,
    pub described_by: Option<String>,
    pub is_subtype_of: Option<String>,
    pub owned_by: Option<String>,
}

pub fn parse_relationship_types_yaml(path: &Path) -> Result<RelationshipTypesFile, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse {:?}: {}", path, e))
}
