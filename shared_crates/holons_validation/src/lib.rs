//! Descriptor-aware validation of completed holon/property/value inputs.
//!
//! Resolve a `HolonValidationContext` once for an unchanged transaction snapshot,
//! assess subjects with an ordered `ValidationCollector`, then consume the collector
//! into an identity-only report. An operational error invalidates the partial pass;
//! callers must discard its collector rather than treat it as an acceptance result.
//! Individual validators remain report-only. `validate_commit_candidates` assesses the
//! complete supplied candidate set before replacing staged outcomes, preserving operational
//! errors separately. No entry point invokes Commit or persists holons.

mod collector;
mod commitments;
mod contexts;
mod handlers;
mod orchestration;
mod registries;
mod report;
mod subjects;
mod validators;

pub use collector::{ValidationCollector, ValidationObservations};
pub use commitments::{ResolvedConstraint, ResolvedValidationBinding};
pub use contexts::{HolonValidationContext, PropertyValidationContext, ValueValidationContext};
pub use orchestration::validate_commit_candidates;
pub use registries::{
    ConstraintTypeKey, RuleOutcome, StaticConstraintHandler, StaticConstraintRegistry,
    StaticRuleHandler, StaticRuleRegistry, ValidationInvocation, ValidationRuleKey,
};
pub use report::CommitValidationReport;
pub use subjects::{HolonValidationSubject, PropertyValidationSubject, ValueValidationSubject};
pub use validators::{validate_holon, validate_property, validate_value};

#[cfg(test)]
mod tests;
