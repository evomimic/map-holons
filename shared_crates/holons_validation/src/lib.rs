//! DescriptorPackage-backed assessment of Commit candidates.
//!
//! `assess_commit_candidates` returns identity-only findings and coverage observations
//! without changing staged content or validation outcomes. `validate_commit_candidates`
//! runs the same assessment, then decides and installs each candidate's `ValidationState`
//! and findings. Operational errors discard the incomplete pass and preserve prior outcomes.
//! No entry point invokes Commit or persists holons.

mod assessment_phase;
mod assessment_support;
mod constraints;
mod diagnostics;
mod dispatch;
mod rules;
pub use assessment_phase::{AssessmentEvent, AssessmentPhase, PreparationMetrics};
mod commit_assessment;
mod commitments;
mod contexts;
mod descriptor_package;
mod graph_only_deltas;
mod key_evidence;
mod structural_diagnoses;
pub use key_evidence::{KeyClaim, KeyEvidence};
mod outcomes;
mod schema_view;
mod subject_assessment;

pub use commit_assessment::{
    assess_commit_candidates, validate_commit_candidates, validate_commit_candidates_with_observer,
    validate_commit_candidates_with_profile,
};
pub use commitments::{ResolvedConstraint, ResolvedValidationBinding};
pub use constraints::declarations::ConstraintDeclarationAssessment;
pub use contexts::ValueValidationContext;
pub use diagnostics::{CommitAssessment, CommitValidationReport};
pub use diagnostics::{ValidationCollector, ValidationObservations};
pub use dispatch::{
    ConstraintTypeKey, RuleOutcome, StaticConstraintHandler, StaticConstraintRegistry,
    StaticRuleHandler, ValidationInvocation, ValidationRuleKey,
};
pub use dispatch::{PreparedRuleSubject, ValueValidationSubject};
pub use rules::descriptor::DescriptorRuleProducts;
pub use rules::schema::SchemaRuleProducts;

#[cfg(test)]
mod subject_assessment_tests;

mod transient_contract;
pub use transient_contract::{
    validate_effective_descriptor_dance_contract, validate_transient_contract,
};
