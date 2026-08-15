use base_types::MapString;
use core_types::HolonError;

/// Schema-backed operator categories understood by the descriptor runtime.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OperatorCategory {
    Equality,
    Ordering,
}

impl OperatorCategory {
    /// Parses the enum variant TypeName stored on operator descriptors.
    pub fn parse(value: &MapString) -> Result<Self, HolonError> {
        match value.0.as_str() {
            "Equality" => Ok(Self::Equality),
            "Ordering" => Ok(Self::Ordering),
            _ => Err(HolonError::UnknownOperatorCategory { value: value.to_string() }),
        }
    }
}
