use serde::{Deserialize, Serialize};
use shared_validation::ValidationError;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, Error, Eq, PartialEq)]
pub enum HolonError {
    #[error("Cache Error: {0}")]
    CacheError(String),
    #[error("Commit Failure {0}")]
    CommitFailure(String),
    #[error(
        "You must remove related holons from {0} relationship before you can delete this holon."
    )]
    DeletionNotAllowed(String),
    #[error("Failed to downcast to {0}")]
    DowncastFailure(String),
    #[error("Multiple {0} found for: {1}")]
    DuplicateError(String, String),
    #[error("{0} field is missing")]
    EmptyField(String),
    #[error("Failed to Borrow {0}")]
    FailedToBorrow(String),
    #[error("Couldn't convert {0} into {1} ")]
    HashConversion(String, String),
    #[error("Holon not found: {0}")]
    HolonNotFound(String),
    #[error("Index {0} into Holons Vector is Out of Range")]
    IndexOutOfRange(String),
    #[error("Invalid HolonReference, {0}")]
    InvalidHolonReference(String),
    #[error("Invalid Transition, {0}")]
    InvalidTransition(String),
    #[error("Invalid Type, {0}")]
    InvalidType(String),
    #[error("Invalid Parameter: {0}")]
    InvalidParameter(String),
    #[error("{0} is not a valid relationship for this source holon type {1}")]
    InvalidRelationship(String, String), // TODO: move this error to ValidationError
    #[error("Updates requires: {0}")]
    InvalidUpdate(String),
    #[error("Miscellaneous error: {0}")]
    Misc(String),
    #[error("{0} relationship is missing StagedCollection")]
    MissingStagedCollection(String),
    #[error("{0} access not allowed while holon is in {1} state")]
    NotAccessible(String, String),
    #[error("{0} Not Implemented")]
    NotImplemented(String),
    #[error("Couldn't convert Record to {0}")]
    RecordConversion(String),
    #[error("to {0}")]
    UnableToAddHolons(String),
    #[error("Unable to cast {0} into expected ValueType: {1}")]
    UnexpectedValueType(String, String),
    #[error("Invalid UTF8: Couldn't convert {0} into {1}")]
    Utf8Conversion(String, String),
    #[error("Validation error: {0}")]
    ValidationError(ValidationError),
    #[error("WasmError {0}")]
    WasmError(String),
}

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
