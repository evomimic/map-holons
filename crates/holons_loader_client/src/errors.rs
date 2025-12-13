//! Loader-client specific error helpers.
//!
//! This module centralizes:
//! - Mapping low-level parsing/validation issues into a single `HolonError`
//!   suitable for returning across the Receptor boundary.
//! - Formatting those issues into human-readable diagnostics for logs or UI.
//!
//! Phase 1 can keep the implementation minimal (e.g., aggregate messages into
//! a single string); future phases can add richer structures if needed.

use core_types::{HolonError, ValidationError};
use std::fmt::Write;

use crate::parser::{ImportFileParsingIssue, ImportFileParsingIssueKind};

/// Convert a list of per-file parsing issues into a single `HolonError`
/// that can be returned from the loader client entrypoint.
///
/// Typical behavior (to be implemented later):
/// - Summarize the number of failing files.
/// - Concatenate or otherwise compress their messages.
/// - Wrap this summary in `HolonError::LoaderParsingError(...)`.
pub fn map_parsing_issues_to_holon_error(issues: &[ImportFileParsingIssue]) -> HolonError {
    if issues.is_empty() {
        return HolonError::LoaderParsingError(
            "Loader parsing failed but no issues were reported".into(),
        );
    }

    let formatted = format_parsing_issues(issues);
    let has_schema_issue = issues
        .iter()
        .any(|issue| matches!(issue.kind, ImportFileParsingIssueKind::SchemaValidationFailure));

    if has_schema_issue {
        return HolonError::ValidationError(ValidationError::JsonSchemaError(formatted));
    }

    HolonError::LoaderParsingError(formatted)
}

/// Render parsing issues into a user-readable, multi-line string.
///
/// This is useful for logging or for attaching a human-facing message to
/// an error holon in a later phase. The exact format is loader-client
/// specific and can evolve independently of the core error codes.
pub fn format_parsing_issues(issues: &[ImportFileParsingIssue]) -> String {
    if issues.is_empty() {
        return "No loader parsing issues reported.".to_string();
    }

    let mut buffer = String::new();
    for (index, issue) in issues.iter().enumerate() {
        if index > 0 {
            buffer.push('\n');
        }

        let kind_label = match issue.kind {
            ImportFileParsingIssueKind::IoFailure => "io_failure",
            ImportFileParsingIssueKind::SchemaValidationFailure => "schema_validation",
            ImportFileParsingIssueKind::JsonDecodingFailure => "json_decoding",
            ImportFileParsingIssueKind::HolonConstructionFailure => "holon_construction",
        };

        let _ =
            write!(&mut buffer, "{}: {}: {}", issue.file_path.display(), kind_label, issue.message);

        if let Some(source) = &issue.source_error {
            let _ = write!(&mut buffer, " (source: {source})");
        }
    }

    buffer
}
