//! Prospective, DescriptorPackage-backed assessment of Commit candidates.
//!
//! `assess_commit_candidates` returns identity-only findings and coverage observations
//! without changing staged content or validation outcomes. `validate_commit_candidates`
//! runs the same assessment, then decides and installs each candidate's `ValidationState`
//! and findings. Operational errors discard the incomplete pass and preserve prior outcomes.
//! No entry point invokes Commit or persists holons.

mod assessment_phase;
mod assessment_support;
pub use assessment_phase::AssessmentPhase;
mod collector;
mod commitments;
mod constraint_declarations;
mod contexts;
mod dependency_groups;
mod descriptor_package;
mod descriptor_rules;
mod handlers;
mod orchestration;
mod prospective;
mod prospective_validation;
mod readiness;
mod registries;
mod report;
mod schema_rules;
mod schema_view;
mod subjects;

pub use collector::{ValidationCollector, ValidationObservations};
pub use commitments::{ResolvedConstraint, ResolvedValidationBinding};
pub use constraint_declarations::{ConstraintDeclarationAssessment, ConstraintDeclarationRoots};
pub use contexts::ValueValidationContext;
pub use descriptor_rules::{ContractKindRoots, DescriptorRuleProducts};
pub use prospective::{
    competing_replacement_findings, resolve_validation_anchor, resolve_validation_anchor_in_view,
};
pub use readiness::{
    assess_commit_candidates, validate_commit_candidates, validate_commit_candidates_with_observer,
};
pub use registries::{
    ConstraintTypeKey, RuleOutcome, StaticConstraintHandler, StaticConstraintRegistry,
    StaticRuleHandler, StaticRuleRegistry, ValidationInvocation, ValidationRuleKey,
};
pub use report::{CommitAssessment, CommitValidationReport};
pub use schema_rules::SchemaRuleProducts;
pub use subjects::{PreparedRuleSubject, ValueValidationSubject};

#[cfg(test)]
mod tests;
