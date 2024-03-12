use hdk::prelude::*;
use thiserror::Error;

#[hdk_entry_helper]
#[derive(Error, Eq, PartialEq, Clone)]
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
    #[error("{0} Not Implemented")]
    NotImplemented(String),
    #[error("{0} relationship is missing StagedCollection")]
    MissingStagedCollection(String),
    #[error("for {0}")]
    FailedToBorrowMutably(String),
    #[error("to {0}")]
    UnableToAddHolons(String),
    #[error("{0} is not a valid relationship for this source holon type {1}")]
    InvalidRelationship(String, String),
    #[error("Cache Error: {0}")]
    CacheError(String),
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

use std::cell::BorrowError;

impl From<BorrowError> for HolonError {
    fn from(error: BorrowError) -> Self {
        HolonError::InvalidHolonReference(format!("Failed to borrow Rc<RefCell<Holon>>: {}", error))
    }
}
