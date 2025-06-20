use crate::parse::type_header::TypeHeader;
use serde::{Deserialize, Serialize};

/// Represents a string-based value type entry in the MAP type system.
///
/// Used when `type_kind: "String"` in the YAML definition.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StringTypeEntry {
    pub type_kind: String, // Always "String"
    pub variant: String,
    pub header: TypeHeader,
    pub type_name: String,
    pub min_length: u32,
    pub max_length: u32,
}
