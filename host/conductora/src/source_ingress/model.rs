use serde::Serialize;

/// A retained UTF-8 snapshot. Identity is the normalized absolute path, not a basename.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SourceFile {
    pub id: String,
    pub path: String,
    pub content: String,
}

/// Discovery failures and exclusions remain visible even when other sources succeed.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceIssueKind {
    Unreadable,
    SymlinkSkipped,
    UnsupportedFile,
    InvalidPath,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SourceIssue {
    pub path: String,
    pub kind: SourceIssueKind,
    pub message: String,
}

/// An empty confirmed selection is distinct from cancellation and may contain issues.
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct SourceDiscovery {
    pub sources: Vec<SourceFile>,
    pub issues: Vec<SourceIssue>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "status", content = "discovery", rename_all = "camelCase")]
pub enum SourceSelection {
    Cancelled,
    Selected(SourceDiscovery),
}
