//! Bounded source locations joined to canonical loader facts, never semantic state.

use serde::Serialize;

/// LSP-compatible UTF-16 position (zero based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePosition {
    pub line: u32,
    pub character: u32,
}

/// Half-open source range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceRange {
    pub fn contains(&self, position: SourcePosition) -> bool {
        self.start <= position && position < self.end
    }
}

/// Identity within a document snapshot. Target ordinals preserve repeated occurrences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LoaderFactIdentity {
    Holon,
    Property { name: String },
    Relationship { name: String, occurrence: usize },
}

/// A source range attached to a fact that actually lowered successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactProvenance {
    pub holon_key: String,
    pub fact: LoaderFactIdentity,
    pub range: SourceRange,
}

/// Parser-owned source event. Unlowered events remain useful for incomplete-buffer outlines.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceEvent {
    pub declaration: usize,
    pub kind: String,
    pub key: Option<String>,
    pub role: Option<String>,
    pub range: SourceRange,
    pub fact: Option<LoaderFactIdentity>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDiagnostic {
    pub message: String,
    pub code: String,
    pub range: SourceRange,
}

/// Transient parse bookkeeping; it is discarded after joining source events to lowered facts.
#[derive(Debug, Clone, Default)]
pub(crate) struct ParseProvenance {
    pub events: Vec<SourceEvent>,
    pub diagnostics: Vec<SourceDiagnostic>,
    pub current: usize,
}
