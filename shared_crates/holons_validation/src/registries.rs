use core_types::{BaseValueKind, HolonError, ValidationSubjectPath};
use type_names::{CoreDefinitionRole, CoreValidationRuleName};

use crate::{
    contexts::SubjectLevel, handlers, ResolvedConstraint, ResolvedValidationBinding,
    ValidationCollector, ValueValidationContext, ValueValidationSubject,
};

/// Canonical fully qualified schema key of a validation rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationRuleKey(pub String);

/// Canonical schema key of a concrete constraint type, never its display name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintTypeKey(pub String);

/// Typed dispatch inputs preserve the downward-only dependency boundary.
pub enum ValidationInvocation<'a> {
    /// Selected prospective facts evaluated by the same subject handlers.
    Prepared {
        code: &'static str,
        binding: &'a ResolvedValidationBinding,
        path: &'a ValidationSubjectPath,
        subject: &'a crate::PreparedRuleSubject,
    },
    /// Aggregate products computed once for a Schema, even when it is unstaged.
    Schema {
        code: &'static str,
        binding: &'a ResolvedValidationBinding,
        path: &'a ValidationSubjectPath,
        products: &'a crate::schema_rules::SchemaRuleProducts,
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

/// Internal configured-constraint evaluator contract; no evaluators are registered.
pub type StaticConstraintHandler = fn(
    &ResolvedConstraint,
    ValueValidationSubject<'_>,
    &ValueValidationContext,
    &mut ValidationCollector,
) -> Result<(), HolonError>;

/// Execution route selected by a built-in rule's registration.
#[derive(Clone, Copy)]
pub(crate) enum RuleImplementation {
    Subject(StaticRuleHandler),
    DescriptorProduct,
    SchemaAggregate(StaticRuleHandler),
}

/// Subject filtering uses prepared classification rather than discovering additional anchors.
#[derive(Clone, Copy)]
pub(crate) enum SubjectApplicability {
    AnySubject,
}

/// Implementation semantics of one rule key; these fields confer no definition identity.
#[derive(Clone, Copy)]
pub(crate) struct RuleRegistration {
    pub implementation: Option<RuleImplementation>,
    pub level: SubjectLevel,
    pub family: CoreDefinitionRole,
    pub declaring: CoreDefinitionRole,
    pub native_kind: Option<BaseValueKind>,
    pub code: &'static str,
    pub prerequisite: bool,
    pub applicability: SubjectApplicability,
}

/// Supplies all built-in rule metadata without resolving a catalog of definitions.
pub(crate) fn registration(name: CoreValidationRuleName) -> RuleRegistration {
    use CoreDefinitionRole as Role;
    use CoreValidationRuleName::*;
    use RuleImplementation::*;
    use SubjectLevel::*;
    match name {
        RequiredPropertyPresence => RuleRegistration {
            implementation: Some(Subject(handlers::required_property_presence)),
            level: Property,
            family: Role::PropertyValidationRule,
            declaring: Role::PropertyType,
            native_kind: None,
            code: "DS-PROP-001",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        NoUndescribedProperties => RuleRegistration {
            implementation: Some(Subject(handlers::no_undescribed_properties)),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::HolonType,
            native_kind: None,
            code: "DS-PROP-003",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        KeyUniqueness => RuleRegistration {
            implementation: Some(Subject(handlers::key_uniqueness)),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::HolonType,
            native_kind: None,
            code: "DS-KEY-006",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesString => RuleRegistration {
            implementation: Some(Subject(handlers::base_value_kind_matches)),
            level: Value,
            family: Role::StringValidationRule,
            declaring: Role::StringValueType,
            native_kind: Some(BaseValueKind::String),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesInteger => RuleRegistration {
            implementation: Some(Subject(handlers::base_value_kind_matches)),
            level: Value,
            family: Role::IntegerValidationRule,
            declaring: Role::IntegerValueType,
            native_kind: Some(BaseValueKind::Integer),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesBoolean => RuleRegistration {
            implementation: Some(Subject(handlers::base_value_kind_matches)),
            level: Value,
            family: Role::BooleanValidationRule,
            declaring: Role::BooleanValueType,
            native_kind: Some(BaseValueKind::Boolean),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesBytes => RuleRegistration {
            implementation: Some(Subject(handlers::base_value_kind_matches)),
            level: Value,
            family: Role::BytesValidationRule,
            declaring: Role::BytesValueType,
            native_kind: Some(BaseValueKind::Bytes),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesEnum => RuleRegistration {
            implementation: Some(Subject(handlers::base_value_kind_matches)),
            level: Value,
            family: Role::EnumValueValidationRule,
            declaring: Role::EnumValueType,
            native_kind: Some(BaseValueKind::Enum),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        AtMostOneDirectParent => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-STRUCT-002",
            prerequisite: true,
            applicability: SubjectApplicability::AnySubject,
        },
        AcyclicExtendsLineage => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-STRUCT-003",
            prerequisite: true,
            applicability: SubjectApplicability::AnySubject,
        },
        ExtendsLineageTerminatesAtTypeDescriptor => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-STRUCT-004",
            prerequisite: true,
            applicability: SubjectApplicability::AnySubject,
        },
        UniqueTypeDescriptorRoot => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-STRUCT-005",
            prerequisite: true,
            applicability: SubjectApplicability::AnySubject,
        },
        LocalInstanceKindAnchorDesignation => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-KIND-001",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        InstanceKindAnchorsAreAbstract => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-KIND-002",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        TypeDescriptorRootKindException => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-KIND-003",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        DescribingCategoryCompatibility => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::HolonType,
            native_kind: None,
            code: "DS-KIND-004",
            prerequisite: true,
            applicability: SubjectApplicability::AnySubject,
        },
        DescriptorMetaTypeCorrespondence => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::HolonType,
            native_kind: None,
            code: "DS-KIND-005",
            prerequisite: true,
            applicability: SubjectApplicability::AnySubject,
        },
        NoInheritedMemberRedeclaration => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-CONTRACT-001",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        UniqueSemanticMemberNames => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-CONTRACT-002",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        WellFormedEffectiveMemberDefinitions => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-CONTRACT-003",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        ContractMemberKindCompatibility => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-CONTRACT-004",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        InheritedValueConstraintNonRelaxation => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-CONSTRAINT-001",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        SchemaDependenciesAcyclic => RuleRegistration {
            implementation: Some(SchemaAggregate(crate::schema_rules::schema_dependencies_acyclic)),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::Schema,
            native_kind: None,
            code: "DS-SCHEMA-001",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        CrossSchemaDependenciesDeclared => RuleRegistration {
            implementation: None,
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::HolonType,
            native_kind: None,
            code: "DS-SCHEMA-002",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
    }
}

/// Internal constraint dispatch; every reached constraint fails closed during subject evaluation.
pub struct StaticConstraintRegistry;

impl StaticConstraintRegistry {
    /// No configured value or cardinality evaluator is registered.
    pub fn lookup(_key: &ConstraintTypeKey) -> Option<StaticConstraintHandler> {
        None
    }
}
