//! DescriptorPackage-backed assessment of Commit candidates.
//!
//! `assess_commit_candidates` returns identity-only findings and coverage observations
//! without changing staged content or validation outcomes. `validate_commit_candidates`
//! runs the same assessment, then decides and installs each candidate's `ValidationState`
//! and findings. Operational errors discard the incomplete pass and preserve prior outcomes.
//! No entry point invokes Commit or persists holons.

mod assessment;
mod commitments;
mod constraints;
mod diagnostics;
mod dispatch;
mod rules;
mod transient_contract;

#[cfg(test)]
mod tests;

pub use assessment::key_evidence::{KeyClaim, KeyEvidence};
pub use assessment::phases::{AssessmentEvent, AssessmentPhase, PreparationMetrics};

pub use assessment::commit::{
    assess_commit_candidates, validate_commit_candidates, validate_commit_candidates_with_observer,
    validate_commit_candidates_with_profile,
};
pub use assessment::support::ValueValidationContext;
pub use commitments::{ResolvedConstraint, ResolvedValidationBinding};
pub use constraints::declarations::ConstraintDeclarationAssessment;
pub use diagnostics::{CommitAssessment, CommitValidationReport};
pub use diagnostics::{ValidationCollector, ValidationObservations};
pub use dispatch::{
    ConstraintTypeKey, RuleOutcome, StaticConstraintHandler, StaticConstraintRegistry,
    StaticRuleHandler, ValidationInvocation, ValidationRuleKey,
};
pub use dispatch::{PreparedRuleSubject, ValueValidationSubject};
pub use rules::descriptor::DescriptorRuleProducts;
pub use rules::schema::SchemaRuleProducts;

pub use transient_contract::{
    validate_effective_descriptor_dance_contract, validate_transient_contract,
};
