use thiserror::Error;
use hdk::prelude::*;
#[hdk_entry_helper]
#[derive(Error, Eq, PartialEq)]
pub enum HolonError {
    #[error("{0} field is missing")]
    EmptyField(String),
    #[error("No Holon Record Found for HolonId {0}")]
    HolonNotFound(String),
    #[error("WasmError {0}")]
    WasmError(String),
    #[error("Couldn't convert Record to {0}")]
    RecordConversion(String),

    // #[error("Element missing its Entry")]
    // EntryMissing,

    // #[error("Wasm Error {0}")]
    // Wasm(WasmError),
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
