use crate::parse::type_header::TypeHeader;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HolonTypeEntry {
    pub type_name: String,
    pub header: TypeHeader,
    pub properties: Vec<String>,
    pub key_properties: Vec<String>,
    #[serde(default)]
    pub source_for: Vec<String>,
}
