use core_types::{BaseValue, BaseValueKind, HolonError, ValidationSubjectPath};
use holons_core::descriptors::value_constraints::{
    ConstraintEvaluationOutcome, ScalarConstraintFamily, ScalarConstraintParameters,
};
use type_names::{CoreDefinitionRole, CoreValidationRuleName};

use crate::{ResolvedValidationBinding, ValidationCollector};

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
        products: &'a crate::rules::schema::SchemaRuleProducts,
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

/// Pure scalar execution over prepared parameters and an actual value.
pub type StaticValueConstraintEvaluator =
    fn(&ScalarConstraintParameters, &BaseValue) -> ConstraintEvaluationOutcome;

/// Subject-specific evaluator selected by the single constraint registry.
#[derive(Clone, Copy)]
pub enum StaticConstraintEvaluator {
    Value(StaticValueConstraintEvaluator),
}

impl StaticConstraintEvaluator {
    /// An implementation for another subject level does not establish support.
    pub fn supports(self, level: SubjectLevel) -> bool {
        matches!((self, level), (Self::Value(_), SubjectLevel::Value))
    }
}

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
            implementation: Some(Subject(crate::rules::conformance::required_property_presence)),
            level: Property,
            family: Role::PropertyValidationRule,
            declaring: Role::PropertyType,
            native_kind: None,
            code: "DS-PROP-001",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        PropertyValueConformance => RuleRegistration {
            implementation: Some(Subject(crate::rules::conformance::property_value_conformance)),
            level: Property,
            family: Role::PropertyValidationRule,
            declaring: Role::PropertyType,
            native_kind: None,
            code: "DS-PROP-002",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        NoUndescribedProperties => RuleRegistration {
            implementation: Some(Subject(crate::rules::conformance::no_undescribed_properties)),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::HolonType,
            native_kind: None,
            code: "DS-PROP-003",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        KeyUniqueness => RuleRegistration {
            implementation: Some(Subject(crate::rules::conformance::key_uniqueness)),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::HolonType,
            native_kind: None,
            code: "DS-KEY-006",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesString => RuleRegistration {
            implementation: Some(Subject(crate::rules::conformance::base_value_kind_matches)),
            level: Value,
            family: Role::StringValidationRule,
            declaring: Role::StringValueType,
            native_kind: Some(BaseValueKind::String),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesInteger => RuleRegistration {
            implementation: Some(Subject(crate::rules::conformance::base_value_kind_matches)),
            level: Value,
            family: Role::IntegerValidationRule,
            declaring: Role::IntegerValueType,
            native_kind: Some(BaseValueKind::Integer),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesBoolean => RuleRegistration {
            implementation: Some(Subject(crate::rules::conformance::base_value_kind_matches)),
            level: Value,
            family: Role::BooleanValidationRule,
            declaring: Role::BooleanValueType,
            native_kind: Some(BaseValueKind::Boolean),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesBytes => RuleRegistration {
            implementation: Some(Subject(crate::rules::conformance::base_value_kind_matches)),
            level: Value,
            family: Role::BytesValidationRule,
            declaring: Role::BytesValueType,
            native_kind: Some(BaseValueKind::Bytes),
            code: "BaseValueKindMismatch",
            prerequisite: false,
            applicability: SubjectApplicability::AnySubject,
        },
        BaseValueKindMatchesEnum => RuleRegistration {
            implementation: Some(Subject(crate::rules::conformance::base_value_kind_matches)),
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
        AbstractPropertyParent => RuleRegistration {
            implementation: Some(DescriptorProduct),
            level: Holon,
            family: Role::HolonValidationRule,
            declaring: Role::MetaTypeDescriptor,
            native_kind: None,
            code: "DS-STRUCT-006",
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
            implementation: Some(SchemaAggregate(
                crate::rules::schema::schema_dependencies_acyclic,
            )),
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
    /// Selects exact concrete scalar types and filters by evaluation subject.
    pub fn lookup(
        key: &ConstraintTypeKey,
        level: SubjectLevel,
    ) -> Option<StaticConstraintEvaluator> {
        ScalarConstraintFamily::for_concrete_type(&key.0)?;
        let evaluator = StaticConstraintEvaluator::Value(ScalarConstraintParameters::evaluate);
        evaluator.supports(level).then_some(evaluator)
    }
}

/// Read-only inputs prepared through the Commit assessment's bound references.
/// Handlers consume these facts without navigating to containing subjects.
pub enum PreparedRuleSubject {
    /// Populated names absent from the selected effective contract.
    Holon {
        undescribed_properties: Vec<core_types::PropertyName>,
        key_claim: Option<crate::KeyClaim>,
    },
    /// A missing property whose minimum and requiredness have already been resolved.
    Property { missing_required: bool, name: String, descriptor_identity: String },
    /// The selected value type's native representation and the populated value kind.
    Value {
        expected: holons_core::ValueDescriptorKind,
        actual: core_types::BaseValueKind,
        descriptor_identity: String,
    },
}

/// Expected invocation boundary, distinct from a schema's authored display labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubjectLevel {
    Holon,
    Property,
    Value,
}
