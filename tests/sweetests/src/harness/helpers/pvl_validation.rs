use holochain::conductor::{api::error::ConductorApiError, CellError};
use holochain::core::workflow::WorkflowError;
use holochain_state::source_chain::SourceChainError;
use std::fmt::Debug;

const APP_VALIDATION_PREFIX: &str = "Validation failed while committing: ";

/// Asserts that an authoring call was rejected with exactly the expected PVL message.
///
/// Holochain wraps an Integrity callback rejection in its authoring-path error types and
/// prefixes the callback message. Keeping that substrate-specific knowledge here lets PVL
/// sweettests assert consensus-visible messages without duplicating brittle error plumbing.
pub fn assert_commit_rejected_with_pvl<T: Debug>(
    result: Result<T, ConductorApiError>,
    expected_message: &str,
) {
    let reason = match result {
        Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) => {
            match *workflow_error {
                WorkflowError::SourceChainError(SourceChainError::InvalidCommit(reason)) => reason,
                other => panic!("expected InvalidCommit, got workflow error {other:?}"),
            }
        }
        Err(other) => panic!("expected InvalidCommit, got conductor error {other:?}"),
        Ok(value) => panic!("expected the commit to be rejected, but it returned {value:?}"),
    };

    assert_eq!(reason, format!("{APP_VALIDATION_PREFIX}{expected_message}"));
}
