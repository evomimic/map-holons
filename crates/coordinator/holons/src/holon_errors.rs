use hdk::prelude::*;
use holochain::conductor::api::error::ConductorApiError;
use thiserror::Error;

#[hdk_entry_helper]
#[derive(Error, Eq, PartialEq)]
pub enum HolonError {
    #[error("{0} field is missing")]
    EmptyField(String),
    #[error("Holon not found: {0}")]
    HolonNotFound(String),
    #[error("WasmError {0}")]
    WasmError(String),
    #[error("Couldn't convert Record to {0}")]
    RecordConversion(String),
    #[error("Invalid HolonReference, {0}")]
    InvalidHolonReference(String),
    // #[error("Wrong type: {0}")]
    // TypeError(String),
    #[error("ConductorApiError {0}")]
    ConductorApiError(String),
}

impl From<WasmError> for HolonError {
    fn from(e: WasmError) -> Self {
        HolonError::WasmError(e.to_string())
    }
}

impl Into<WasmError> for HolonError {
    fn into(self) -> WasmError {
        wasm_error!("HolonError {:?}", self.to_string())
    }
}

impl From<ConductorApiError> for HolonError {
    fn from(e: ConductorApiError) -> Self {
        HolonError::ConductorApiError(e.to_string())
    }
}
