use crate::descriptors::value_descriptor_subtypes::helpers::{
    unsupported_operator_error, value_kind_mismatch,
};
use crate::descriptors::{
    accessor_helpers, CaseMatching, Descriptor, OperatorDescriptor, ValueDescriptor,
    ValueDescriptorKind,
};
use base_types::{BaseValue, BaseValueKind};
use core_types::HolonError;

/// Supported operator semantics, classified from an operator descriptor's lineage.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OperatorKind {
    Equals,
    LessThan,
    /// Literal substring containment under the operator's case-matching policy.
    Contains(CaseMatching),
}

impl OperatorKind {
    /// Number of authored operands after the subject that this semantics consumes.
    fn authored_operand_count(self) -> usize {
        match self {
            Self::Equals | Self::LessThan | Self::Contains(_) => 1,
        }
    }

    /// Whether this semantics is defined for a subject of `kind`.
    fn applies_to(self, kind: &ValueDescriptorKind) -> bool {
        use BaseValueKind::*;
        match (self, kind) {
            (Self::Equals, ValueDescriptorKind::BaseValue(_)) => true,
            (Self::LessThan, ValueDescriptorKind::BaseValue(Integer | String)) => true,
            (Self::Contains(_), ValueDescriptorKind::BaseValue(String)) => true,
            _ => false,
        }
    }
}

/// An operator resolved once against a subject ValueType and reused per invocation.
///
/// Resolution reads descriptors: affordance, operator kind, arity, ordered operand
/// specifications, and each position's required ValueType. Invocation reads none,
/// so a sort or filter can apply the same resolved operator to many values.
#[derive(Debug, Clone)]
pub struct ResolvedOperator {
    operator: OperatorDescriptor,
    subject_type: ValueDescriptor,
    subject_kind: ValueDescriptorKind,
    kind: OperatorKind,
    operand_value_types: Vec<ValueDescriptor>,
    operand_kinds: Vec<ValueDescriptorKind>,
}

impl ResolvedOperator {
    /// Resolves `operator` for subjects of `subject_type`.
    ///
    /// The operator must be afforded by the subject type (by descriptor identity),
    /// classify as a supported kind applicable to the subject's value kind, and
    /// declare exactly `Arity - 1` operand specifications matching that kind.
    pub(crate) fn resolve(
        subject_type: &ValueDescriptor,
        operator: &OperatorDescriptor,
    ) -> Result<Self, HolonError> {
        let subject_kind = subject_type.supported_value_kind()?;
        if !subject_type.supports_operator(operator)? {
            return Err(unsupported_operator_error(subject_type.holon(), operator)?);
        }

        let operator_name = operator.operator_name()?.to_string();
        let kind = operator.kind()?;
        if !kind.applies_to(&subject_kind) {
            return Err(unsupported_operator_error(subject_type.holon(), operator)?);
        }

        let arity = usize::from(operator.arity()?);
        let specifications = operator.operand_specifications()?;
        if arity == 0 || specifications.len() != arity - 1 {
            return Err(HolonError::InvalidOperatorMetadata {
                operator: operator_name,
                reason: format!(
                    "Arity {arity} requires Arity - 1 operand specifications, found {}",
                    specifications.len()
                ),
            });
        }
        if specifications.len() != kind.authored_operand_count() {
            return Err(HolonError::InvalidOperatorMetadata {
                operator: operator_name,
                reason: format!(
                    "{kind:?} takes {} authored operand(s), but {} are specified",
                    kind.authored_operand_count(),
                    specifications.len()
                ),
            });
        }

        let mut operand_value_types = Vec::with_capacity(specifications.len());
        let mut operand_kinds = Vec::with_capacity(specifications.len());
        for specification in &specifications {
            let value_type = specification.required_value_type(&operator_name, subject_type)?;
            operand_kinds.push(value_type.supported_value_kind()?);
            operand_value_types.push(value_type);
        }

        Ok(Self {
            operator: operator.clone(),
            subject_type: subject_type.clone(),
            subject_kind,
            kind,
            operand_value_types,
            operand_kinds,
        })
    }

    /// Returns the resolved operator descriptor.
    pub fn operator(&self) -> &OperatorDescriptor {
        &self.operator
    }

    /// Returns the subject ValueType this operator was resolved against.
    pub fn subject_type(&self) -> &ValueDescriptor {
        &self.subject_type
    }

    /// Returns the subject's value kind, classified once at resolution.
    pub(crate) fn subject_kind(&self) -> &ValueDescriptorKind {
        &self.subject_kind
    }

    /// Returns the classified operator semantics.
    pub fn kind(&self) -> OperatorKind {
        self.kind
    }

    /// Returns the required ValueType of each authored operand, in position order.
    pub fn operand_value_types(&self) -> &[ValueDescriptor] {
        &self.operand_value_types
    }

    /// Applies the operator to a subject value and its ordered authored operands.
    ///
    /// Checks the operand count and each value's runtime representation only.
    /// Authored-literal validation belongs to query definition validation, and
    /// stored subject values are never revalidated against configured constraints.
    pub fn invoke(&self, subject: &BaseValue, operands: &[BaseValue]) -> Result<bool, HolonError> {
        if operands.len() != self.operand_kinds.len() {
            return Err(HolonError::OperandCountMismatch {
                operator: self.operator.operator_name()?.to_string(),
                expected: self.operand_kinds.len(),
                found: operands.len(),
            });
        }
        require_representation(&self.subject_type, &self.subject_kind, subject)?;
        for ((operand, kind), value_type) in
            operands.iter().zip(&self.operand_kinds).zip(&self.operand_value_types)
        {
            require_representation(value_type, kind, operand)?;
        }
        self.subject_type.evaluate_operator(self, subject, operands)
    }
}

/// Requires `value` to use the native representation of `kind`.
fn require_representation(
    value_type: &ValueDescriptor,
    kind: &ValueDescriptorKind,
    value: &BaseValue,
) -> Result<(), HolonError> {
    match kind {
        ValueDescriptorKind::BaseValue(expected) if value.kind() == *expected => Ok(()),
        ValueDescriptorKind::BaseValue(expected) => {
            Err(value_kind_mismatch(value_type.holon(), &expected.to_string(), value))
        }
        ValueDescriptorKind::AnyBaseValue => Ok(()),
        ValueDescriptorKind::ValueArray | ValueDescriptorKind::Unsupported(_) => {
            Err(HolonError::WrongDescriptorKind {
                expected: "scalar value type".to_string(),
                found: format!("{kind:?}"),
                descriptor: accessor_helpers::descriptor_label(value_type.holon()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support::{
        build_context, core_operator, new_descriptor_holon, new_kind_descriptor, stage_operator,
        OperatorFixture,
    };
    use crate::reference_layer::{HolonReference, WritableHolon};
    use base_types::{MapBytes, MapInteger, MapString};
    use std::sync::Arc;
    use type_names::{CoreOperatorTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

    /// A staged string anchor (the Contains substring type) and the canonical operators.
    struct World {
        context: Arc<TransactionContext>,
        string_type: HolonReference,
        equals: HolonReference,
        less_than: HolonReference,
        contains: HolonReference,
    }

    fn world() -> World {
        let context = build_context();
        let string_type: HolonReference = context
            .mutation()
            .stage_new_holon(new_kind_descriptor(&context, "StringValueType", None, true).unwrap())
            .unwrap()
            .into();
        let equals = core_operator(&context, CoreOperatorTypeName::EqualsOperator, None).unwrap();
        let less_than =
            core_operator(&context, CoreOperatorTypeName::LessThanOperator, None).unwrap();
        let contains =
            core_operator(&context, CoreOperatorTypeName::ContainsOperator, Some(&string_type))
                .unwrap();
        World { context, string_type, equals, less_than, contains }
    }

    impl World {
        /// A subject value type of `kind_anchor`'s kind that affords `operators`.
        fn subject(&self, kind_anchor: &str, operators: &[&HolonReference]) -> ValueDescriptor {
            let anchor: HolonReference = if kind_anchor == "StringValueType" {
                self.string_type.clone()
            } else {
                self.context
                    .mutation()
                    .stage_new_holon(
                        new_kind_descriptor(&self.context, kind_anchor, None, true).unwrap(),
                    )
                    .unwrap()
                    .into()
            };
            let mut subject = new_kind_descriptor(
                &self.context,
                &format!("Subject{kind_anchor}"),
                Some(&anchor),
                false,
            )
            .unwrap();
            subject
                .add_related_holons(
                    CoreRelationshipTypeName::AffordsOperator,
                    operators.iter().map(|operator| (*operator).clone()).collect(),
                )
                .unwrap();
            ValueDescriptor::from_holon(subject.into())
        }

        fn resolve(
            &self,
            operator: &HolonReference,
            subject: &ValueDescriptor,
        ) -> Result<ResolvedOperator, HolonError> {
            OperatorDescriptor::from_holon(operator.clone()).resolve_for_subject(subject)
        }
    }

    fn string(value: &str) -> BaseValue {
        BaseValue::StringValue(MapString(value.to_string()))
    }

    #[test]
    fn contains_matches_case_folded_literal_substrings() {
        let world = world();
        let title = world.subject("StringValueType", &[&world.contains]);
        let contains = world.resolve(&world.contains, &title).unwrap();

        assert_eq!(contains.kind(), OperatorKind::Contains(CaseMatching::UnicodeDefaultCaseFold));
        assert_eq!(contains.operand_value_types()[0].holon(), &world.string_type);
        assert!(contains.invoke(&string("Straße"), &[string("STRASSE")]).unwrap());
        assert!(contains.invoke(&string("Graph Theory"), &[string("graph")]).unwrap());
        assert!(contains.invoke(&string("anything"), &[string("")]).unwrap());
        assert!(!contains.invoke(&string("Graph"), &[string("G.*")]).unwrap());
        assert!(!contains.invoke(&string("Graph"), &[string("tree")]).unwrap());
    }

    #[test]
    fn contains_requires_exactly_one_string_operand() {
        let world = world();
        let title = world.subject("StringValueType", &[&world.contains]);
        let contains = world.resolve(&world.contains, &title).unwrap();

        for operands in [vec![], vec![string("a"), string("b")]] {
            assert!(matches!(
                contains.invoke(&string("abc"), &operands),
                Err(HolonError::OperandCountMismatch { expected: 1, found, .. })
                    if found == operands.len()
            ));
        }
        assert!(matches!(
            contains.invoke(&string("abc"), &[BaseValue::IntegerValue(MapInteger(1))]),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "String" && found == "Integer"
        ));
        assert!(matches!(
            contains.invoke(&BaseValue::IntegerValue(MapInteger(1)), &[string("1")]),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "String" && found == "Integer"
        ));
    }

    #[test]
    fn contains_is_unsupported_on_non_string_subjects_even_when_afforded() {
        let world = world();
        let count = world.subject("IntegerValueType", &[&world.contains]);
        assert!(matches!(
            world.resolve(&world.contains, &count),
            Err(HolonError::UnsupportedOperator { operator, .. }) if operator == "ContainsOperator"
        ));
    }

    #[test]
    fn equals_and_less_than_compare_against_the_subject_type() {
        let world = world();
        let title = world.subject("StringValueType", &[&world.equals, &world.less_than]);
        let equals = world.resolve(&world.equals, &title).unwrap();
        let less_than = world.resolve(&world.less_than, &title).unwrap();

        assert_eq!(equals.operand_value_types()[0].holon(), title.holon());
        assert!(equals.invoke(&string("a"), &[string("a")]).unwrap());
        assert!(!equals.invoke(&string("a"), &[string("A")]).unwrap());
        assert!(less_than.invoke(&string("a"), &[string("b")]).unwrap());

        let bytes = world.subject("BytesValueType", &[&world.equals]);
        let bytes_equals = world.resolve(&world.equals, &bytes).unwrap();
        let value = BaseValue::BytesValue(MapBytes(vec![1, 2]));
        assert!(bytes_equals.invoke(&value, &[value.clone()]).unwrap());
        assert!(matches!(
            world.resolve(&world.less_than, &bytes),
            Err(HolonError::UnsupportedOperator { .. })
        ));
    }

    #[test]
    fn extension_operator_executes_with_its_supported_ancestor_semantics() {
        let world = world();
        let case_sensitive = stage_operator(
            &world.context,
            OperatorFixture {
                type_name: "CaseSensitiveContainsOperator",
                category: "Matching",
                case_matching: Some("CaseSensitive"),
                operand_type: Some(&world.string_type),
                extends: Some(&world.contains),
            },
        )
        .unwrap();
        let inheriting = stage_operator(
            &world.context,
            OperatorFixture {
                type_name: "InheritingContainsOperator",
                category: "Matching",
                case_matching: None,
                operand_type: Some(&world.string_type),
                extends: Some(&world.contains),
            },
        )
        .unwrap();
        let title =
            world.subject("StringValueType", &[&world.contains, &case_sensitive, &inheriting]);

        let resolved = world.resolve(&case_sensitive, &title).unwrap();
        assert_eq!(resolved.kind(), OperatorKind::Contains(CaseMatching::CaseSensitive));
        assert!(resolved.invoke(&string("Graph"), &[string("Gra")]).unwrap());
        assert!(!resolved.invoke(&string("Graph"), &[string("graph")]).unwrap());

        // An extension that declares no policy inherits its ancestor's.
        let resolved = world.resolve(&inheriting, &title).unwrap();
        assert_eq!(resolved.kind(), OperatorKind::Contains(CaseMatching::UnicodeDefaultCaseFold));
    }

    #[test]
    fn an_extension_operator_must_be_afforded_itself() {
        let world = world();
        let case_sensitive = stage_operator(
            &world.context,
            OperatorFixture {
                type_name: "CaseSensitiveContainsOperator",
                category: "Matching",
                case_matching: Some("CaseSensitive"),
                operand_type: Some(&world.string_type),
                extends: Some(&world.contains),
            },
        )
        .unwrap();
        let title = world.subject("StringValueType", &[&world.contains]);
        assert!(matches!(
            world.resolve(&case_sensitive, &title),
            Err(HolonError::UnsupportedOperator { operator, .. })
                if operator == "CaseSensitiveContainsOperator"
        ));
    }

    #[test]
    fn an_unrelated_same_named_operator_has_no_supported_semantics() {
        let world = world();
        let rogue: HolonReference =
            new_descriptor_holon(&world.context, "rogue-equals", "EqualsOperator", "Operator")
                .unwrap()
                .into();
        let title = world.subject("StringValueType", &[&world.equals]);

        assert!(matches!(
            OperatorDescriptor::from_holon(rogue.clone()).kind(),
            Err(HolonError::NotImplemented(_))
        ));
        // Affordance is by identity: the canonical Equals does not afford the rogue.
        assert!(matches!(
            world.resolve(&rogue, &title),
            Err(HolonError::UnsupportedOperator { operator, .. }) if operator == "EqualsOperator"
        ));
    }

    #[test]
    fn contains_metadata_requires_a_known_case_matching_policy() {
        for (case_matching, expected_unknown) in [(None, false), (Some("Loose"), true)] {
            let context = build_context();
            let string_type: HolonReference = context
                .mutation()
                .stage_new_holon(
                    new_kind_descriptor(&context, "StringValueType", None, true).unwrap(),
                )
                .unwrap()
                .into();
            let contains = stage_operator(
                &context,
                OperatorFixture {
                    type_name: "ContainsOperator",
                    category: "Matching",
                    case_matching,
                    operand_type: Some(&string_type),
                    extends: None,
                },
            )
            .unwrap();
            let kind = OperatorDescriptor::from_holon(contains).kind();
            if expected_unknown {
                assert!(
                    matches!(kind, Err(HolonError::UnknownCaseMatching { value }) if value == "Loose")
                );
            } else {
                assert!(matches!(kind, Err(HolonError::InvalidOperatorMetadata { .. })));
            }
        }
    }

    #[test]
    fn specification_count_must_equal_arity_minus_one() {
        let world = world();
        let title = world.subject("StringValueType", &[&world.equals]);
        let HolonReference::Staged(mut equals) = world.equals.clone() else {
            panic!("canonical operator fixture is staged");
        };
        equals.with_property_value(CorePropertyTypeName::Arity, MapInteger(3)).unwrap();

        assert!(matches!(
            world.resolve(&world.equals, &title),
            Err(HolonError::InvalidOperatorMetadata { operator, .. }) if operator == "EqualsOperator"
        ));
    }
}
