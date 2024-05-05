pub use validation_error::*;
pub mod validation_error;

// Public module, part of the crate's public API
pub mod validation_helpers;

// Re-exporting key functions/types for ease of use
pub use validation_helpers::*;

pub enum ValidationResult {
    Valid,
    Invalid(Vec<ValidationError>),
}
