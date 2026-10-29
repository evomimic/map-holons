use std::sync::Arc;

use holons_core::core_shared_objects::transactions::TransactionContext;

/// Transaction guard shared by prepared subject assessments.
pub struct ValueValidationContext {
    pub(crate) context: Arc<TransactionContext>,
}

impl ValueValidationContext {
    /// Creates the guard without discovering schema definitions.
    pub fn new(context: &Arc<TransactionContext>) -> Self {
        Self { context: Arc::clone(context) }
    }
}

/// Expected invocation boundary, distinct from a schema's authored display labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubjectLevel {
    Holon,
    Property,
    Value,
}
