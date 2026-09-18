//! Identity-only semantic findings shared across runtime and transport boundaries.

use serde::{Deserialize, Serialize};

/// A semantic finding, separate from an operational persistence error.
///
/// Findings cross transport boundaries unchanged and must never contain bound runtime
/// references. Subjects and descriptors are represented only by serialized identities.
/// Stable machine interpretation uses `kind`, `rule_key`, and the rule-specific `code`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitValidationViolation {
    pub kind: CommitValidationViolationKind,
    pub rule_key: Option<String>,
    pub severity: ValidationSeverity,
    pub subject: ValidationSubjectPath,
    pub descriptor_identity: Option<String>,
    /// Actionable local diagnostic text, not a consensus-visible canonical message.
    pub message: String,
}

/// Machine-readable classification of a semantic finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitValidationViolationKind {
    NoDescriptor,
    UnsupportedValidationRule,
    UnsupportedConstraintType { constraint_identity: String, constraint_type_identity: String },
    RuleViolation { code: String },
    UnresolvedLocalDependency,
    RelationshipCoordinationRequired,
}

/// Serialized identity and path of the subject; never a bound runtime handle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSubjectPath {
    Holon { holon_identity: String },
    Property { holon_identity: String, name: String },
    Value { holon_identity: String, property: String },
    Relationship { source_identity: String, name: String, target_identity: String },
    Transaction,
}

/// Dependency-safe mirror of the Core schema's ValidationSeverity enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn findings_preserve_sdk_json_contract_in_both_directions() {
        // Explicit wire fixtures protect the hand-maintained SDK mirror from serde shape changes.
        let kinds = [
            (CommitValidationViolationKind::NoDescriptor, json!("NoDescriptor")),
            (
                CommitValidationViolationKind::UnsupportedValidationRule,
                json!("UnsupportedValidationRule"),
            ),
            (
                CommitValidationViolationKind::UnsupportedConstraintType {
                    constraint_identity: "constraint".into(),
                    constraint_type_identity: "constraint-type".into(),
                },
                json!({"UnsupportedConstraintType": {
                    "constraint_identity": "constraint",
                    "constraint_type_identity": "constraint-type"
                }}),
            ),
            (
                CommitValidationViolationKind::RuleViolation { code: "DS-PROP-001".into() },
                json!({"RuleViolation": {"code": "DS-PROP-001"}}),
            ),
            (
                CommitValidationViolationKind::UnresolvedLocalDependency,
                json!("UnresolvedLocalDependency"),
            ),
            (
                CommitValidationViolationKind::RelationshipCoordinationRequired,
                json!("RelationshipCoordinationRequired"),
            ),
        ];
        let subjects = [
            (
                ValidationSubjectPath::Holon { holon_identity: "subject".into() },
                json!({"Holon": {"holon_identity": "subject"}}),
            ),
            (
                ValidationSubjectPath::Property {
                    holon_identity: "subject".into(),
                    name: "Name".into(),
                },
                json!({"Property": {"holon_identity": "subject", "name": "Name"}}),
            ),
            (
                ValidationSubjectPath::Value {
                    holon_identity: "subject".into(),
                    property: "Name".into(),
                },
                json!({"Value": {"holon_identity": "subject", "property": "Name"}}),
            ),
            (
                ValidationSubjectPath::Relationship {
                    source_identity: "source".into(),
                    name: "RelatedTo".into(),
                    target_identity: "target".into(),
                },
                json!({"Relationship": {"source_identity": "source", "name": "RelatedTo", "target_identity": "target"}}),
            ),
            (ValidationSubjectPath::Transaction, json!("Transaction")),
        ];
        for (kind, kind_json) in kinds {
            for (subject, subject_json) in &subjects {
                for (severity, severity_json) in [
                    (ValidationSeverity::Info, json!("Info")),
                    (ValidationSeverity::Warning, json!("Warning")),
                    (ValidationSeverity::Error, json!("Error")),
                ] {
                    for (rule_key, descriptor_identity) in [
                        (None, None),
                        (Some("required-property"), None),
                        (None, Some("descriptor")),
                        (Some("required-property"), Some("descriptor")),
                    ] {
                        let finding = CommitValidationViolation {
                            kind: kind.clone(),
                            rule_key: rule_key.map(str::to_owned),
                            severity,
                            subject: subject.clone(),
                            descriptor_identity: descriptor_identity.map(str::to_owned),
                            message: "Supply the required property".into(),
                        };
                        let expected = json!({
                            "kind": kind_json,
                            "rule_key": rule_key,
                            "severity": severity_json,
                            "subject": subject_json,
                            "descriptor_identity": descriptor_identity,
                            "message": "Supply the required property"
                        });
                        assert_eq!(serde_json::to_value(&finding).unwrap(), expected);
                        assert_eq!(
                            serde_json::from_value::<CommitValidationViolation>(expected).unwrap(),
                            finding
                        );
                    }
                }
            }
        }
    }
}
