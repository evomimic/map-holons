//! Loader-client specific error helpers.
//!
//! This module centralizes:
//! - Mapping low-level parsing/validation issues into a single `HolonError`
//!   suitable for returning across the Receptor boundary.
//! - Formatting those issues into human-readable diagnostics for logs or UI.
//!
//! Structured findings and readable summaries travel together across the existing error boundary.

use core_types::HolonError;
use std::fmt::Write;

use crate::parser::{ImportFileParsingIssue, ImportFileParsingIssueKind};

/// Preserve each parser finding while retaining a readable summary for existing callers.
pub fn map_parsing_issues_to_holon_error(issues: &[ImportFileParsingIssue]) -> HolonError {
    HolonError::LoaderParsingError(core_types::LoaderParsingFailure {
        message: if issues.is_empty() {
            "Loader parsing failed but no issues were reported".into()
        } else {
            format_parsing_issues(issues)
        },
        issues: issues
            .iter()
            .map(|issue| core_types::LoaderParsingIssue {
                filename: issue.file_path.to_string_lossy().into_owned(),
                kind: issue.kind.clone(),
                message: issue.message.clone(),
                location: issue.location.clone(),
                source_error: issue.source_error.clone().map(Box::new),
            })
            .collect(),
    })
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
            ImportFileParsingIssueKind::JsonDecodingFailure => "json_decoding",
            ImportFileParsingIssueKind::StructuralValidationFailure => "structural_validation",
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
