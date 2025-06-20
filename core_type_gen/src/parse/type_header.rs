use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TypeHeader {
    pub descriptor_name: String,
    pub description: String,
    pub label: String,
    pub is_dependent: bool,
    pub is_value_type: bool,
    pub described_by: Option<String>,
    pub is_subtype_of: Option<String>,
    pub owned_by: Option<String>,
}
