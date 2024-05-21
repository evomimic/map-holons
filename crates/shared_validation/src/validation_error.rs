use hdk::prelude::*;
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
}

impl From<WasmError> for ValidationError {
    fn from(error: WasmError) -> Self {
        ValidationError::WasmError(error.to_string())
    }
}

impl Into<WasmError> for ValidationError {
    fn into(self) -> WasmError {
        wasm_error!("ValidationError {:?}", self.to_string())
    }
}
