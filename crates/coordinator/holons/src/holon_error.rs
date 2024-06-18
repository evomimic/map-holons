use hdk::prelude::*;
use shared_validation::ValidationError;
use thiserror::Error;

#[hdk_entry_helper]
#[derive(Error, Eq, PartialEq, Clone)]
pub enum HolonError {
    #[error("{0} field is missing")]
    EmptyField(String),
    #[error("{0} parameter is not valid")]
    InvalidParameter(String),
    #[error("Holon not found: {0}")]
    HolonNotFound(String),
    #[error("Commit Failure {0}")]
    CommitFailure(String),
    #[error("WasmError {0}")]
    WasmError(String),
    #[error("Couldn't convert Record to {0}")]
    RecordConversion(String),
    #[error("Couldn't convert {0} into {1} ")]
    HashConversion(String, String),
    #[error("Invalid HolonReference, {0}")]
    InvalidHolonReference(String),
    #[error("Index {0} into Holons Vector is Out of Range")]
    IndexOutOfRange(String),
    #[error("{0} Not Implemented")]
    NotImplemented(String),
    #[error("{0} relationship is missing StagedCollection")]
    MissingStagedCollection(String),
    #[error("Failed to Borrow {0}")]
    FailedToBorrow(String),
    #[error("to {0}")]
    UnableToAddHolons(String),
    #[error("{0} is not a valid relationship for this source holon type {1}")]
    InvalidRelationship(String, String), // TODO: move this error to ValidationError
    #[error("Cache Error: {0}")]
    CacheError(String),
    #[error("Validation error: {0}")]
    ValidationError(ValidationError),
    #[error("{0} access not allowed while holon is in {1} state")]
    NotAccessible(String, String),
    #[error("Unable to cast {0} into expected ValueType: {1}")]
    UnexpectedValueType(String, String),
    #[error("Invalid UTF8: Couldn't convert {0} into {1}")]
    Utf8Conversion(String, String),
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

use shared_types_holon::{BaseType, BaseValue, MapString, ValueType};
use std::cell::BorrowError;

impl From<BorrowError> for HolonError {
    fn from(error: BorrowError) -> Self {
        HolonError::InvalidHolonReference(format!("Failed to borrow Rc<RefCell<Holon>>: {}", error))
    }
}

impl HolonError {
    pub fn combine_errors(errors: Vec<HolonError>) -> String {
        let mut combined = String::new();
        for (i, error) in errors.into_iter().enumerate() {
            if i > 0 {
                combined.push_str(", ");
            }
            combined.push_str(&error.to_string());
        }
        combined
    }
}
