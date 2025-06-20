use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct SchemaDefinitionFile {
    pub schema: SchemaBlock, // ← THIS is the missing field
    pub type_files: Vec<String>,
}

// impl From<&SchemaDefinitionFile> for SchemaBlock {
//     fn from(def: &SchemaDefinitionFile) -> Self {
//         def.schema.clone()
//     }
// }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SchemaBlock {
    pub type_name: String,
    pub described_by: RefWrapper,
    pub properties: SchemaProperties,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SchemaProperties {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RefWrapper {
    #[serde(rename = "$ref")]
    pub reference: RefDetails,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RefDetails {
    pub type_name: String,
    pub schema: Option<String>,
    pub space: Option<String>,
}
