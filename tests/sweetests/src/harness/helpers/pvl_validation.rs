use holochain::conductor::{api::error::ConductorApiError, CellError};
use holochain::core::ribosome::error::RibosomeError;
use holochain::core::workflow::WorkflowError;
use holochain_state::source_chain::SourceChainError;
use holochain_wasmer_common::{WasmError, WasmErrorInner};
use std::{error::Error, fmt::Debug};

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

/// Asserts that coordinator preflight rejected a typed write with the exact PVL message.
///
/// This deliberately recognizes only the guest-error path produced before a host write. Keeping
/// it separate from [`assert_commit_rejected_with_pvl`] prevents a preflight failure from being
/// mistaken for evidence that the Integrity callback rejected an authored operation.
pub fn assert_preflight_rejected_with_pvl<T: Debug>(
    result: Result<T, ConductorApiError>,
    expected_message: &str,
) {
    let runtime_error = match result {
        Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) => {
            match *workflow_error {
                WorkflowError::RibosomeError(RibosomeError::WasmRuntimeError(runtime_error)) => {
                    runtime_error
                }
                other => panic!("expected coordinator preflight guest error, got {other:?}"),
            }
        }
        Err(other) => panic!("expected coordinator preflight guest error, got {other:?}"),
        Ok(value) => panic!("expected coordinator preflight rejection, but it returned {value:?}"),
    };

    let wasm_error = runtime_error
        .source()
        .and_then(|source| source.downcast_ref::<WasmError>())
        .unwrap_or_else(|| panic!("expected WasmError source, got {runtime_error:?}"));
    match &wasm_error.error {
        WasmErrorInner::Guest(message) => assert_eq!(message, expected_message),
        other => panic!("expected guest PVL error, got {other:?}"),
    }
}
