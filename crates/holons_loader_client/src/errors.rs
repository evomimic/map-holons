//! Loader-client specific error helpers.
//!
//! This module centralizes:
//! - Mapping low-level parsing/validation issues into a single `HolonError`
//!   suitable for returning across the Receptor boundary.
//! - Formatting those issues into human-readable diagnostics for logs or UI.
//!
//! Phase 1 can keep the implementation minimal (e.g., aggregate messages into
//! a single string); future phases can add richer structures if needed.

use core_types::HolonError;

use crate::parser::ImportFileParsingIssue;

/// Convert a list of per-file parsing issues into a single `HolonError`
/// that can be returned from the loader client entrypoint.
///
/// Typical behavior (to be implemented later):
/// - Summarize the number of failing files.
/// - Concatenate or otherwise compress their messages.
/// - Wrap this summary in `HolonError::LoaderParsingError(...)`.
pub fn map_parsing_issues_to_holon_error(issues: &[ImportFileParsingIssue]) -> HolonError {
    todo!()
}

/// Render parsing issues into a user-readable, multi-line string.
///
/// This is useful for logging or for attaching a human-facing message to
/// an error holon in a later phase. The exact format is loader-client
/// specific and can evolve independently of the core error codes.
pub fn format_parsing_issues(issues: &[ImportFileParsingIssue]) -> String {
    todo!()
}
