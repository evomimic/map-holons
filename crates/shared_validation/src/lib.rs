// Private module, not exposed as part of the public API
// mod utils;

// Public modules, part of the crate's public API
pub mod properties;
pub mod relationships;
pub mod holon_validation;

// Re-exporting key functions/types for ease of use
pub use properties::validate_property;
pub use relationships::{validate_relationship_cardinality, validate_relationship_target};
pub use holon_validation::validate_holon;

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub message: String,
}

impl ValidationError {
    pub fn new(message: &str) -> Self {
        Self { message: message.to_string() }
    }
}

pub enum ValidationResult {
    Valid,
    Invalid(Vec<ValidationError>),
}