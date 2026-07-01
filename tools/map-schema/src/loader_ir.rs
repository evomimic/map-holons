//! Generic loader-oriented intermediate representation for MAP schema tooling.
//!
//! This module models canonical import content independently of either TDL or
//! JSON syntax. The current milestone uses it as the backend target for
//! `Schema IR -> Loader IR -> JSON` compilation.

use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoaderDocument {
    pub meta: LoaderMeta,
    pub holons: Vec<LoaderHolon>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoaderMeta {
    pub generator: Option<String>,
    pub generated_at: Option<String>,
    pub export_mode: Option<String>,
    pub source_files: Vec<String>,
    pub load_with: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoaderHolon {
    pub key: String,
    pub descriptor_type: String,
    pub properties: serde_json::Map<String, Value>,
    pub relationships: Vec<LoaderRelationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoaderRelationship {
    pub name: String,
    pub targets: Vec<LoaderReference>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoaderReference {
    pub target: String,
}
