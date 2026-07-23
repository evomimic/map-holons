pub mod holon_node_envelope;
// Public module, part of the crate's public API
pub mod pvl_limits_v1;
pub mod validation_helpers;

// Re-exporting key functions/types for ease of use
pub use holon_node_envelope::*;
pub use validation_helpers::*;

pub use integrity_core_types::{PvlField, PvlMalformedReason, PvlViolation};

use core_types::ValidationError;

pub enum ValidationResult {
    Valid,
    Invalid(Vec<ValidationError>),
}
