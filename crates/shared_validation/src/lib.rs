pub use validation_error::*;
pub mod validation_error;

// We may eventually want these modules to be private
pub mod properties;
pub mod relationships;

// Public module, part of the crate's public API
pub mod holon_validation;

// Re-exporting key functions/types for ease of use
pub use properties::validate_property;
//pub use relationships::{validate_relationship_existence, validate_relationship_properties};
pub use holon_validation::validate_holon_comprehensive;

pub enum ValidationResult {
    Valid,
    Invalid(Vec<ValidationError>),
}
