use hdk::prelude::WasmError; // Ensure you have this import for WasmError
use serde::{Deserialize, Serialize};
use std::fmt;
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

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl From<WasmError> for ValidationError {
    fn from(error: WasmError) -> Self {
        ValidationError::WasmError(error.to_string())
    }
}

impl Into<WasmError> for ValidationError {
    fn into(self) -> WasmError {
        match self {
            ValidationError::WasmError(msg) => wasm_error!(msg),
            // Handle other variants if necessary, converting them to a WasmError representation
            _ => wasm_error!("Unhandled ValidationError type"),
        }
    }
}
