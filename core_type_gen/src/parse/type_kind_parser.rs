use serde::Serialize;
use std::path::Path;

/// General trait for parsing a YAML file containing type definitions
/// and producing both an enum and JSON spec files.
pub trait ParseTypeKind: Sized {
    /// A single item in the spec, e.g. a HolonType or PropertyType
    type TypeSpecItem: Serialize;

    fn type_kind_name() -> &'static str;

    /// Parse from the given YAML file path
    fn parse_yaml(path: &Path) -> Result<Self, String>;

    /// Return (name, item) pairs for JSON generation
    fn type_spec_items(&self) -> Vec<(String, &Self::TypeSpecItem)>;
}
