//! Required assessment-anchor resolution and schema compatibility diagnostics.
use core_types::HolonError;
use holons_core::{
    core_shared_objects::transactions::TransactionContext, resolve_assessment_anchor,
    HolonReference,
};
use std::sync::Arc;

/// Missing Core vocabulary requires bootstrap/upgrade; unreadable inputs remain operational errors.
#[derive(Debug)]
pub enum AnchorResolutionError {
    /// A mandatory Core definition is absent.
    SchemaIncompatible { missing_anchor: String },
    /// Reference resolution failed for an existing or ambiguous input.
    Operational(HolonError),
}
impl std::fmt::Display for AnchorResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaIncompatible { missing_anchor } => write!(
                f,
                "Required validation anchor {missing_anchor} is missing; \
                 bootstrap or upgrade the Core Schema before retrying."
            ),
            Self::Operational(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for AnchorResolutionError {}
impl From<HolonError> for AnchorResolutionError {
    fn from(error: HolonError) -> Self {
        Self::Operational(error)
    }
}
impl From<AnchorResolutionError> for HolonError {
    fn from(error: AnchorResolutionError) -> Self {
        match error {
            AnchorResolutionError::Operational(error) => error,
            error @ AnchorResolutionError::SchemaIncompatible { .. } => {
                HolonError::CommitFailure(error.to_string())
            }
        }
    }
}
/// Resolves assessment anchors without substituting version-producing successors.
pub fn resolve_validation_anchor(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<HolonReference, AnchorResolutionError> {
    resolve_assessment_anchor(context, key).map_err(|error| match error {
        HolonError::HolonNotFound(_) => {
            AnchorResolutionError::SchemaIncompatible { missing_anchor: key.into() }
        }
        error => AnchorResolutionError::Operational(error),
    })
}
