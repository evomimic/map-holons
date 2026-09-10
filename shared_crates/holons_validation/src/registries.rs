use core_types::{HolonError, ValidationSubjectPath};
use holons_core::HolonDescriptor;
use type_names::CoreValidationRuleName;

use crate::{
    handlers, HolonValidationSubject, PropertyValidationContext, PropertyValidationSubject,
    ResolvedConstraint, ResolvedValidationBinding, ValidationCollector, ValueValidationContext,
    ValueValidationSubject,
};

/// Canonical fully qualified schema key of a validation rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationRuleKey(pub String);

/// Canonical schema key of a concrete constraint type, never its display name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintTypeKey(pub String);

/// Typed dispatch inputs preserve the downward-only dependency boundary.
pub enum ValidationInvocation<'a> {
    /// Whole-holon policy against its governing descriptor.
    Holon {
        /// Selected effective binding, including declaration provenance.
        binding: &'a ResolvedValidationBinding,
        /// Bound subject being assessed.
        subject: HolonValidationSubject<'a>,
        /// Governing descriptor discovered at bootstrap navigation.
        descriptor: &'a HolonDescriptor,
        /// Identity-only diagnostic path.
        path: &'a ValidationSubjectPath,
    },
    /// Complete-contract property, even when its value is absent.
    Property {
        /// Selected effective binding.
        binding: &'a ResolvedValidationBinding,
        /// Property-local subject without a containing holon.
        subject: PropertyValidationSubject<'a>,
        /// Minimum enforcement and lower-level services.
        context: &'a PropertyValidationContext<'a>,
    },
    /// Native value without upward navigation to a property or holon.
    Value {
        /// Selected effective binding.
        binding: &'a ResolvedValidationBinding,
        /// Populated value and selected descriptor.
        subject: ValueValidationSubject<'a>,
        /// Immutable descriptor roots used for kind classification.
        context: &'a ValueValidationContext,
    },
}

/// Whether subsequent type-specific evaluation is meaningful for the subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleOutcome {
    /// This rule does not prevent subsequent assessment.
    Continue,
    /// Native kind mismatch prevents type-specific evaluation of this value.
    StopValueEvaluation,
}

/// Fixed Rust handler; operational failures invalidate the enclosing assessment.
pub type StaticRuleHandler =
    fn(ValidationInvocation<'_>, &mut ValidationCollector) -> Result<RuleOutcome, HolonError>;

/// Internal configured-constraint evaluator contract; C1 registers no evaluators.
pub type StaticConstraintHandler = fn(
    &ResolvedConstraint,
    ValueValidationSubject<'_>,
    &ValueValidationContext,
    &mut ValidationCollector,
) -> Result<(), HolonError>;

/// Static rule dispatch, independent of schema implementation or plugin metadata.
pub struct StaticRuleRegistry;

impl StaticRuleRegistry {
    /// Looks up exactly the initial seven canonical rule identities.
    pub fn lookup(key: &ValidationRuleKey) -> Option<StaticRuleHandler> {
        use CoreValidationRuleName::*;
        match CoreValidationRuleName::from_key(&key.0)? {
            RequiredPropertyPresence => Some(handlers::required_property_presence),
            NoUndescribedProperties => Some(handlers::no_undescribed_properties),
            BaseValueKindMatchesString
            | BaseValueKindMatchesInteger
            | BaseValueKindMatchesBoolean
            | BaseValueKindMatchesBytes
            | BaseValueKindMatchesEnum => Some(handlers::base_value_kind_matches),
        }
    }
}

/// Internal constraint dispatch; every reached constraint fails closed in C1.
pub struct StaticConstraintRegistry;

impl StaticConstraintRegistry {
    /// No configured value or cardinality evaluator belongs to this cohort.
    pub fn lookup(_key: &ConstraintTypeKey) -> Option<StaticConstraintHandler> {
        None
    }
}
