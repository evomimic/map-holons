pub mod smartlink_tag;
// Public module, part of the crate's public API
pub mod validation_helpers;

// Re-exporting key functions/types for ease of use
pub use smartlink_tag::*;
pub use validation_helpers::*;

use core_types::ValidationError;

pub enum ValidationResult {
    Valid,
    Invalid(Vec<ValidationError>),
}
