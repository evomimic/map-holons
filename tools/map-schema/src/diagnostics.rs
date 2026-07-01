//! Diagnostics produced while building or validating the semantic schema model.
//!
//! These diagnostics are intentionally source-agnostic: JSON import code and future TDL parsing can
//! attach their own [`Origin`] values while sharing the same validation vocabulary.

use crate::schema_ir::{DescriptorKind, Origin, ReferenceRole};
use std::fmt;

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

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
        }
    }
}

impl fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSymbol { key } => {
                write!(f, "duplicate symbol key `{key}`")
            }
            Self::UnresolvedReference { role, target } => {
                write!(f, "unresolved {role} reference `{target}`")
            }
            Self::WrongDescriptorKind { role, target, actual, expected } => {
                write!(
                    f,
                    "{role} reference `{target}` resolved to {actual}, expected {}",
                    format_expected_kinds(expected)
                )
            }
            Self::MissingRequiredField { descriptor, field } => {
                write!(f, "descriptor `{descriptor}` is missing required field `{field}`")
            }
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.severity, self.kind)?;
        if let Some(origin) = &self.origin {
            write!(f, " ({})", format_origin(origin))?;
        }
        Ok(())
    }
}

pub fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_expected_kinds(expected: &[DescriptorKind]) -> String {
    expected
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_origin(origin: &Origin) -> String {
    match (&origin.file_path, origin.line, origin.column) {
        (Some(path), Some(line), Some(column)) => format!("{}:{line}:{column}", path.display()),
        (Some(path), Some(line), None) => format!("{}:{line}", path.display()),
        (Some(path), None, None) => path.display().to_string(),
        _ => format!("{:?}", origin.source_kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_ir::{Origin, SourceKind};
    use std::path::PathBuf;

    #[test]
    fn formats_unresolved_reference_with_origin() {
        let diagnostic = Diagnostic::error(
            DiagnosticKind::UnresolvedReference {
                role: ReferenceRole::InstanceRelationship,
                target: "DeclaredRelationshipType".to_string(),
            },
            Some(Origin::tdl_file("schema-src/example.tdl", Some(12), Some(3))),
        );

        assert_eq!(
            diagnostic.to_string(),
            "error: unresolved InstanceRelationships reference `DeclaredRelationshipType` (schema-src/example.tdl:12:3)"
        );
    }

    #[test]
    fn formats_multiple_diagnostics_as_newline_separated_list() {
        let diagnostics = vec![
            Diagnostic::error(
                DiagnosticKind::DuplicateSymbol { key: "Name.PropertyType".to_string() },
                Some(Origin::json_file("fixture.json")),
            ),
            Diagnostic::error(
                DiagnosticKind::MissingRequiredField {
                    descriptor: "Owns.RelationshipType".to_string(),
                    field: "SourceType".to_string(),
                },
                Some(Origin {
                    source_kind: SourceKind::Generated,
                    file_path: Some(PathBuf::from("generated.tdl")),
                    line: None,
                    column: None,
                }),
            ),
        ];

        assert_eq!(
            format_diagnostics(&diagnostics),
            "error: duplicate symbol key `Name.PropertyType` (fixture.json)\nerror: descriptor `Owns.RelationshipType` is missing required field `SourceType` (generated.tdl)"
        );
    }
}
