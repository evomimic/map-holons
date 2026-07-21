//! Diagnostics produced while building or validating the Canonical Holon IR.
//!
//! These diagnostics are intentionally source-agnostic: JSON import code and future TDL parsing can
//! attach their own [`Origin`] values while sharing the same validation vocabulary.

use crate::schema_ir::{DescriptorKind, Origin, ReferenceRole};
use std::fmt;

/// Responsibility boundary that produced a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLayer {
    Syntax,
    IrStructural,
    DeclarationShape,
    DescriptorKind,
    ReferenceSymbol,
    SchemaAware,
    SemanticFidelity,
    RuntimeLoaderBoundary,
}

/// Severity assigned to a semantic diagnostic.
///
/// Diagnostics use compiler-style severity so source adapters and CLIs can present a single stream
/// of schema feedback even when some issues are non-fatal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    /// Issue that prevents a valid semantic model/index from being trusted.
    Error,
    /// Issue that should be surfaced but may not block projection.
    Warning,
}

/// Validation issue detected while deriving symbols or resolving references.
///
/// Diagnostic kinds describe semantic failures after source parsing has already happened. Parser
/// errors should stay in their source-specific layer and only use these variants once a partial
/// semantic model exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// Source syntax was malformed while parsing authored content.
    InvalidSyntax { context: String, message: String },
    /// A required syntax element was missing from authored content.
    MissingSyntaxElement { context: String, expected: String },
    /// Two schemas/descriptors produced the same canonical symbol key.
    DuplicateSymbol { key: String },
    /// A reference target could not be resolved against the derived symbol index.
    UnresolvedReference { role: ReferenceRole, target: String },
    /// A reference resolved, but the target descriptor kind is not valid for the reference role.
    WrongDescriptorKind {
        role: ReferenceRole,
        target: String,
        actual: DescriptorKind,
        expected: Vec<DescriptorKind>,
    },
    /// A descriptor is missing a semantic field required by its descriptor kind.
    MissingRequiredField { descriptor: String, field: String },
    /// Two attachments of the same role use the same local member name.
    DuplicateLocalMember { descriptor: String, role: ReferenceRole, name: String },
    /// An inheritance edge crosses projected TypeKind boundaries.
    TypeKindMismatch { descriptor: String, target: String, actual: String, expected: String },
    /// An `Extends` edge has a source or target that is not described by `TypeDescriptor`.
    ExtendsEndpointNotType { descriptor: String, endpoint: String },
    /// Relationship cardinality bounds are present but invalid.
    InvalidCardinalityBounds { descriptor: String, min: i64, max: i64 },
    /// A relationship descriptor is missing the required paired inverse metadata.
    MissingInverseRelationship { descriptor: String },
    /// Multiple relationship descriptors claim the same inverse pairing.
    DuplicateInverseRelationship { descriptor: String, inverse: String },
    /// Relationship pair metadata does not point back consistently.
    InverseRelationshipMismatch { descriptor: String, inverse: String, expected: String },
    /// A relationship pair resolved to the wrong declared/inverse flavor.
    WrongRelationshipFlavor {
        role: ReferenceRole,
        descriptor: String,
        target: String,
        actual: String,
        expected: String,
    },
    /// An `Extends` chain contains a cycle.
    InheritanceCycle { descriptor: String, target: String },
    /// A descriptor graph relationship violates kernel-owned structural cardinality.
    DescriptorRelationshipCardinality {
        holon: String,
        relationship: String,
        actual: usize,
        maximum: usize,
    },
    /// The Canonical Holon IR graph could not provide a target required by descriptor semantics.
    DescriptorGraphAccess {
        holon: String,
        relationship: String,
        target: Option<String>,
        message: String,
    },
    /// A holon omits a property required by its effective descriptor.
    MissingConformanceProperty { holon: String, property: String },
    /// A closed descriptor does not declare an authored property.
    AdditionalConformanceProperty { holon: String, property: String },
    /// A property value does not satisfy its resolved value descriptor.
    InvalidConformanceValue { holon: String, property: String, value: String, expected: String },
    /// A closed descriptor does not declare an authored relationship.
    AdditionalConformanceRelationship { holon: String, relationship: String },
    /// A relationship target collection violates its descriptor-provided bounds.
    ConformanceRelationshipCardinality {
        holon: String,
        relationship: String,
        actual: usize,
        minimum: usize,
        maximum: usize,
    },
    /// No effective key rule could be derived for a descriptor key.
    MissingEffectiveKeyRule { descriptor: String },
    /// A resolved effective key rule is not one of the supported canonical rules.
    UnsupportedKeyRule { descriptor: String, key_rule: String },
    /// The selected key rule does not have the semantic inputs it requires.
    MissingKeyRuleInput { descriptor: String, key_rule: String, field: String },
    /// The generated key for a descriptor does not match the authored/imported key.
    AuthoredKeyMismatch { descriptor: String, expected: String, actual: String },
}

/// A semantic diagnostic with optional source origin metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub layer: DiagnosticLayer,
    pub kind: DiagnosticKind,
    pub origin: Option<Origin>,
}

impl Diagnostic {
    /// Creates an error diagnostic with optional source origin metadata.
    pub fn error(layer: DiagnosticLayer, kind: DiagnosticKind, origin: Option<Origin>) -> Self {
        Self { severity: DiagnosticSeverity::Error, layer, kind, origin }
    }

    /// Creates a warning diagnostic with optional source origin metadata.
    pub fn warning(layer: DiagnosticLayer, kind: DiagnosticKind, origin: Option<Origin>) -> Self {
        Self { severity: DiagnosticSeverity::Warning, layer, kind, origin }
    }
}

impl fmt::Display for DiagnosticLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax => write!(f, "syntax"),
            Self::IrStructural => write!(f, "ir_structural"),
            Self::DeclarationShape => write!(f, "declaration_shape"),
            Self::DescriptorKind => write!(f, "descriptor_kind"),
            Self::ReferenceSymbol => write!(f, "reference_symbol"),
            Self::SchemaAware => write!(f, "schema_aware"),
            Self::SemanticFidelity => write!(f, "semantic_fidelity"),
            Self::RuntimeLoaderBoundary => write!(f, "runtime_loader_boundary"),
        }
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
            Self::InvalidSyntax { context, message } => {
                write!(f, "invalid syntax in `{context}`: {message}")
            }
            Self::MissingSyntaxElement { context, expected } => {
                write!(f, "missing syntax element `{expected}` in `{context}`")
            }
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
            Self::DuplicateLocalMember { descriptor, role, name } => {
                write!(f, "descriptor `{descriptor}` has duplicate {role} member `{name}`")
            }
            Self::TypeKindMismatch { descriptor, target, actual, expected } => {
                write!(
                    f,
                    "descriptor `{descriptor}` cannot extend `{target}` because projected TypeKind `{actual}` does not match `{expected}`"
                )
            }
            Self::ExtendsEndpointNotType { descriptor, endpoint } => {
                write!(
                    f,
                    "descriptor `{descriptor}` cannot extend through non-type endpoint `{endpoint}`"
                )
            }
            Self::InvalidCardinalityBounds { descriptor, min, max } => {
                write!(f, "descriptor `{descriptor}` has invalid cardinality bounds `{min}..{max}`")
            }
            Self::MissingInverseRelationship { descriptor } => {
                write!(f, "descriptor `{descriptor}` is missing its required inverse relationship")
            }
            Self::DuplicateInverseRelationship { descriptor, inverse } => {
                write!(
                    f,
                    "descriptor `{descriptor}` conflicts with another relationship over inverse `{inverse}`"
                )
            }
            Self::InverseRelationshipMismatch { descriptor, inverse, expected } => {
                write!(
                    f,
                    "descriptor `{descriptor}` points to inverse `{inverse}` but expected a back-reference to `{expected}`"
                )
            }
            Self::WrongRelationshipFlavor { role, descriptor, target, actual, expected } => {
                write!(
                    f,
                    "descriptor `{descriptor}` has {role} target `{target}` with flavor `{actual}`, expected `{expected}`"
                )
            }
            Self::InheritanceCycle { descriptor, target } => {
                write!(
                    f,
                    "descriptor `{descriptor}` participates in an Extends cycle through `{target}`"
                )
            }
            Self::DescriptorRelationshipCardinality { holon, relationship, actual, maximum } => {
                write!(
                    f,
                    "holon `{holon}` has {actual} `{relationship}` targets; expected at most {maximum}"
                )
            }
            Self::DescriptorGraphAccess { holon, relationship, target, message } => {
                write!(f, "cannot traverse `{relationship}` for holon `{holon}`")?;
                if let Some(target) = target {
                    write!(f, " through target `{target}`")?;
                }
                write!(f, ": {message}")
            }
            Self::MissingConformanceProperty { holon, property } => {
                write!(f, "holon `{holon}` is missing required property `{property}`")
            }
            Self::AdditionalConformanceProperty { holon, property } => {
                write!(f, "holon `{holon}` has undeclared property `{property}`")
            }
            Self::InvalidConformanceValue { holon, property, value, expected } => write!(
                f,
                "holon `{holon}` property `{property}` has invalid value `{value}`; expected {expected}"
            ),
            Self::AdditionalConformanceRelationship { holon, relationship } => {
                write!(f, "holon `{holon}` has undeclared relationship `{relationship}`")
            }
            Self::ConformanceRelationshipCardinality {
                holon,
                relationship,
                actual,
                minimum,
                maximum,
            } => write!(
                f,
                "holon `{holon}` has {actual} `{relationship}` targets; expected {minimum}..{maximum}"
            ),
            Self::MissingEffectiveKeyRule { descriptor } => {
                write!(f, "descriptor `{descriptor}` has no effective key rule")
            }
            Self::UnsupportedKeyRule { descriptor, key_rule } => {
                write!(f, "descriptor `{descriptor}` resolved unsupported key rule `{key_rule}`")
            }
            Self::MissingKeyRuleInput { descriptor, key_rule, field } => {
                write!(
                    f,
                    "descriptor `{descriptor}` uses key rule `{key_rule}` but is missing required input `{field}`"
                )
            }
            Self::AuthoredKeyMismatch { descriptor, expected, actual } => {
                write!(
                    f,
                    "descriptor `{descriptor}` has key `{actual}` but effective key rule generates `{expected}`"
                )
            }
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]: {}", self.severity, self.layer, self.kind)?;
        if let Some(origin) = &self.origin {
            write!(f, " ({})", format_origin(origin))?;
        }
        Ok(())
    }
}

/// Formats diagnostics as a newline-separated text block for CLI/test output.
pub fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
}

fn format_expected_kinds(expected: &[DescriptorKind]) -> String {
    expected.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
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
            DiagnosticLayer::ReferenceSymbol,
            DiagnosticKind::UnresolvedReference {
                role: ReferenceRole::InstanceRelationship,
                target: "DeclaredRelationshipType".to_string(),
            },
            Some(Origin::tdl_file("schema-src/example.tdl", Some(12), Some(3))),
        );

        assert_eq!(
            diagnostic.to_string(),
            "error[reference_symbol]: unresolved InstanceRelationships reference `DeclaredRelationshipType` (schema-src/example.tdl:12:3)"
        );
    }

    #[test]
    fn formats_multiple_diagnostics_as_newline_separated_list() {
        let diagnostics = vec![
            Diagnostic::error(
                DiagnosticLayer::Syntax,
                DiagnosticKind::InvalidSyntax {
                    context: "schema".to_string(),
                    message: "unexpected token".to_string(),
                },
                Some(Origin::tdl_file("broken.tdl", Some(1), Some(1))),
            ),
            Diagnostic::error(
                DiagnosticLayer::ReferenceSymbol,
                DiagnosticKind::DuplicateSymbol { key: "Name.PropertyType".to_string() },
                Some(Origin::json_file("fixture.json")),
            ),
            Diagnostic::error(
                DiagnosticLayer::SchemaAware,
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
            "error[syntax]: invalid syntax in `schema`: unexpected token (broken.tdl:1:1)\nerror[reference_symbol]: duplicate symbol key `Name.PropertyType` (fixture.json)\nerror[schema_aware]: descriptor `Owns.RelationshipType` is missing required field `SourceType` (generated.tdl)"
        );
    }
}
