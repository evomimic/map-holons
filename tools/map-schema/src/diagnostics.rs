//! Diagnostics produced while building or validating the semantic schema model.
//!
//! These diagnostics are intentionally source-agnostic: JSON import code and future TDL parsing can
//! attach their own [`Origin`] values while sharing the same validation vocabulary.

use crate::semantic::{DescriptorKind, Origin, ReferenceRole};

/// Severity assigned to a semantic diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// Validation issue detected while deriving symbols or resolving references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    DuplicateSymbol {
        key: String,
    },
    UnresolvedReference {
        role: ReferenceRole,
        target: String,
    },
    WrongDescriptorKind {
        role: ReferenceRole,
        target: String,
        actual: DescriptorKind,
        expected: Vec<DescriptorKind>,
    },
    MissingRequiredField {
        descriptor: String,
        field: String,
    },
}

/// A semantic diagnostic with optional source origin metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub kind: DiagnosticKind,
    pub origin: Option<Origin>,
}

impl Diagnostic {
    pub fn error(kind: DiagnosticKind, origin: Option<Origin>) -> Self {
        Self { severity: DiagnosticSeverity::Error, kind, origin }
    }
}
