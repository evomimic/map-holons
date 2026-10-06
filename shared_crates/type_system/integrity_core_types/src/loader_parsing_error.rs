//! Transport-safe parser evidence; no filesystem or runtime handles are retained.
use crate::HolonError;
use serde::{Deserialize, Serialize};

/// Original category assigned by the host loader parser.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum LoaderParsingIssueKind {
    /// Any failure to read or open the import file.
    IoFailure,
    /// Raw JSON decoding errors (malformed JSON or unexpected shape).
    JsonDecodingFailure,
    /// Generated JSON violates loader-owned structural constraints.
    StructuralValidationFailure,
    /// Failure constructing transient holons or relationships in the loader graph.
    HolonConstructionFailure,
}

/// Coordinates reported by decoding, relative to the original UTF-8 input.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct LoaderParsingLocation {
    /// One-based line number.
    pub line: usize,
    /// serde_json's one-based byte column (zero is possible at early EOF).
    pub column: usize,
}

/// A parser finding that does not require a staged subject or response holon.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct LoaderParsingIssue {
    pub filename: String,
    pub kind: LoaderParsingIssueKind,
    pub message: String,
    pub location: Option<LoaderParsingLocation>,
    pub source_error: Option<Box<HolonError>>,
}

/// Structured preparation failure with a readable fallback summary.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct LoaderParsingFailure {
    pub message: String,
    pub issues: Vec<LoaderParsingIssue>,
}

impl std::fmt::Display for LoaderParsingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
