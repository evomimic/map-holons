use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationError {
    #[error("Property error: {0}")]
    PropertyError(String),

    #[error("Relationship error: {0}")]
    RelationshipError(String),

    #[error("Descriptor error: {0}")]
    DescriptorError(String),

    #[error("Wasm error: {0}")]
    WasmError(String),

    #[error("JSON Schema validation error: {0}")]
    JsonSchemaError(String),
}

