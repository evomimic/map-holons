use crate::descriptors::value_descriptor_subtypes::helpers::{
    unsupported_operator_error, value_kind_mismatch,
};
use crate::descriptors::{
    accessor_helpers, CaseMatching, Descriptor, OperatorDescriptor, ValueDescriptor,
    ValueDescriptorKind,
};
use base_types::{BaseValue, BaseValueKind};
use core_types::HolonError;
use std::cmp::Ordering;

/// Supported operator semantics, classified from an operator descriptor's lineage.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OperatorKind {
    Equals,
    /// An ordered comparison of the subject against one comparison value.
    Ordered(OrderedComparison),
    /// Literal substring containment under the operator's case-matching policy.
    Contains(CaseMatching),
}

/// The ordered comparisons, each relating the subject to its comparison value
/// under the subject value type's own ordering.
///
/// Each comparison is answered directly from one [`Ordering`]; none is the
/// negation of another, so the predicate layer's absence rule (an absent
/// optional value never matches) holds for all of them alike.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OrderedComparison {
    /// Subject strictly before the comparison value.
    LessThan,
    /// Subject strictly after the comparison value.
    GreaterThan,
    /// Subject before or equal to the comparison value.
    LessThanOrEqual,
    /// Subject after or equal to the comparison value.
    GreaterThanOrEqual,
}

impl OrderedComparison {
    /// Whether a subject ordered `ordering` relative to the comparison value
    /// satisfies this comparison.
    pub fn holds(self, ordering: Ordering) -> bool {
        match self {
            Self::LessThan => ordering == Ordering::Less,
            Self::GreaterThan => ordering == Ordering::Greater,
            Self::LessThanOrEqual => ordering != Ordering::Greater,
            Self::GreaterThanOrEqual => ordering != Ordering::Less,
        }
    }
}

impl OperatorKind {
    /// Number of authored operands after the subject that this semantics consumes.
    fn authored_operand_count(self) -> usize {
        match self {
            Self::Equals | Self::Ordered(_) | Self::Contains(_) => 1,
        }
    }

    /// Whether this semantics is defined for a subject of `kind`.
    ///
    /// Ordering is defined for integers and strings only. Other kinds do not
    /// acquire it from their representation or from equality support.
    fn applies_to(self, kind: &ValueDescriptorKind) -> bool {
        use BaseValueKind::*;
        match (self, kind) {
            (Self::Equals, ValueDescriptorKind::BaseValue(_)) => true,
            (Self::Ordered(_), ValueDescriptorKind::BaseValue(Integer | String)) => true,
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
    use base_types::{MapBytes, MapEnumValue, MapInteger, MapString};
    use std::sync::Arc;
    use type_names::{CoreOperatorTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

    /// A staged string anchor (the Contains substring type) and the canonical operators.
    struct World {
        context: Arc<TransactionContext>,
        string_type: HolonReference,
        equals: HolonReference,
        less_than: HolonReference,
        greater_than: HolonReference,
        less_than_or_equal: HolonReference,
        greater_than_or_equal: HolonReference,
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
        let ordered = |name| core_operator(&context, name, None).unwrap();
        let less_than = ordered(CoreOperatorTypeName::LessThanOperator);
        let greater_than = ordered(CoreOperatorTypeName::GreaterThanOperator);
        let less_than_or_equal = ordered(CoreOperatorTypeName::LessThanOrEqualOperator);
        let greater_than_or_equal = ordered(CoreOperatorTypeName::GreaterThanOrEqualOperator);
        let contains =
            core_operator(&context, CoreOperatorTypeName::ContainsOperator, Some(&string_type))
                .unwrap();
        World {
            context,
            string_type,
            equals,
            less_than,
            greater_than,
            less_than_or_equal,
            greater_than_or_equal,
            contains,
        }
    }

    impl World {
        /// The four ordered operators with their expected classification.
        fn ordering(&self) -> [(&HolonReference, OrderedComparison); 4] {
            [
                (&self.less_than, OrderedComparison::LessThan),
                (&self.greater_than, OrderedComparison::GreaterThan),
                (&self.less_than_or_equal, OrderedComparison::LessThanOrEqual),
                (&self.greater_than_or_equal, OrderedComparison::GreaterThanOrEqual),
            ]
        }

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

    fn integer(value: i64) -> BaseValue {
        BaseValue::IntegerValue(MapInteger(value))
    }

    /// Results for a subject before, equal to, and after the comparison value.
    fn boundary_results(comparison: OrderedComparison) -> [bool; 3] {
        match comparison {
            OrderedComparison::LessThan => [true, false, false],
            OrderedComparison::GreaterThan => [false, false, true],
            OrderedComparison::LessThanOrEqual => [true, true, false],
            OrderedComparison::GreaterThanOrEqual => [false, true, true],
        }
    }

    #[test]
    fn each_ordered_comparison_answers_its_own_boundary() {
        for comparison in [
            OrderedComparison::LessThan,
            OrderedComparison::GreaterThan,
            OrderedComparison::LessThanOrEqual,
            OrderedComparison::GreaterThanOrEqual,
        ] {
            let results = [Ordering::Less, Ordering::Equal, Ordering::Greater]
                .map(|ordering| comparison.holds(ordering));
            assert_eq!(results, boundary_results(comparison), "{comparison:?}");
        }
    }

    #[test]
    fn ordered_operators_compare_integers_and_strings_against_the_subject_type() {
        let world = world();
        let operators: Vec<&HolonReference> =
            world.ordering().iter().map(|(operator, _)| *operator).collect();
        let count = world.subject("IntegerValueType", &operators);
        let title = world.subject("StringValueType", &operators);

        for (operator, comparison) in world.ordering() {
            for (subject, values) in [
                (&count, [integer(4), integer(5), integer(6)]),
                (&title, [string("l"), string("m"), string("n")]),
            ] {
                let resolved = world.resolve(operator, subject).unwrap();
                assert_eq!(resolved.kind(), OperatorKind::Ordered(comparison));
                assert_eq!(resolved.operand_value_types().len(), 1);
                assert_eq!(resolved.operand_value_types()[0].holon(), subject.holon());
                let results = values.clone().map(|value| {
                    resolved.invoke(&value, std::slice::from_ref(&values[1])).unwrap()
                });
                assert_eq!(results, boundary_results(comparison), "{comparison:?} {values:?}");
            }
        }
    }

    #[test]
    fn integer_ordering_is_numeric_at_the_signed_extremes() {
        let world = world();
        let operators: Vec<&HolonReference> =
            world.ordering().iter().map(|(operator, _)| *operator).collect();
        let count = world.subject("IntegerValueType", &operators);
        let [less_than, greater_than, less_than_or_equal, greater_than_or_equal] =
            world.ordering().map(|(operator, _)| world.resolve(operator, &count).unwrap());
        let (min, max) = (integer(i64::MIN), integer(i64::MAX));

        assert!(less_than.invoke(&min, &[max.clone()]).unwrap());
        assert!(greater_than.invoke(&max, &[min.clone()]).unwrap());
        assert!(!greater_than.invoke(&min, &[max.clone()]).unwrap());
        assert!(less_than_or_equal.invoke(&max, &[max.clone()]).unwrap());
        assert!(greater_than_or_equal.invoke(&min, &[min.clone()]).unwrap());
        assert!(less_than.invoke(&integer(-1), &[integer(0)]).unwrap());
        assert!(greater_than.invoke(&integer(0), &[integer(-1)]).unwrap());
        assert!(greater_than.invoke(&integer(1), &[min]).unwrap());
        assert!(less_than.invoke(&integer(-1), &[max]).unwrap());
    }

    #[test]
    fn string_ordering_is_case_sensitive_ordinal_and_unfolded() {
        let world = world();
        let operators: Vec<&HolonReference> =
            world.ordering().iter().map(|(operator, _)| *operator).collect();
        let title = world.subject("StringValueType", &operators);
        let [less_than, greater_than, less_than_or_equal, greater_than_or_equal] =
            world.ordering().map(|(operator, _)| world.resolve(operator, &title).unwrap());

        // Empty strings and prefixes.
        assert!(less_than.invoke(&string(""), &[string("a")]).unwrap());
        assert!(greater_than_or_equal.invoke(&string(""), &[string("")]).unwrap());
        assert!(less_than.invoke(&string("Atlas"), &[string("Atlases")]).unwrap());
        // Case: every ASCII uppercase letter precedes every lowercase one.
        assert!(less_than.invoke(&string("Zebra"), &[string("apple")]).unwrap());
        assert!(greater_than.invoke(&string("b"), &[string("B")]).unwrap());
        // Non-ASCII values order by code point, and no folding applies.
        assert!(greater_than.invoke(&string("é"), &[string("z")]).unwrap());
        assert!(greater_than.invoke(&string("Straße"), &[string("STRASSE")]).unwrap());
        assert!(!less_than_or_equal.invoke(&string("Straße"), &[string("STRASSE")]).unwrap());
        assert!(greater_than.invoke(&string("ß"), &[string("ss")]).unwrap());
    }

    #[test]
    fn ordered_operators_require_one_operand_of_the_subject_representation() {
        let world = world();
        let count = world.subject("IntegerValueType", &[&world.greater_than_or_equal]);
        let resolved = world.resolve(&world.greater_than_or_equal, &count).unwrap();

        for operands in [vec![], vec![integer(1), integer(2)]] {
            assert!(matches!(
                resolved.invoke(&integer(1), &operands),
                Err(HolonError::OperandCountMismatch { expected: 1, found, .. })
                    if found == operands.len()
            ));
        }
        assert!(matches!(
            resolved.invoke(&integer(1), &[string("1")]),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Integer" && found == "String"
        ));
        assert!(matches!(
            resolved.invoke(&string("1"), &[integer(1)]),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Integer" && found == "String"
        ));
    }

    #[test]
    fn one_ordered_affordance_does_not_grant_the_others() {
        let world = world();
        let rank = world.subject("IntegerValueType", &[&world.equals, &world.less_than]);
        world.resolve(&world.less_than, &rank).expect("LessThan is afforded");
        for operator in
            [&world.greater_than, &world.less_than_or_equal, &world.greater_than_or_equal]
        {
            assert!(matches!(
                world.resolve(operator, &rank),
                Err(HolonError::UnsupportedOperator { .. })
            ));
        }
    }

    #[test]
    fn ordering_is_unsupported_outside_integers_and_strings_even_when_afforded() {
        let world = world();
        let operators: Vec<&HolonReference> =
            world.ordering().iter().map(|(operator, _)| *operator).collect();
        for kind_anchor in ["BooleanValueType", "BytesValueType", "EnumValueType"] {
            let subject = world.subject(kind_anchor, &[&[&world.equals][..], &operators].concat());
            world.resolve(&world.equals, &subject).expect("Equals applies to every scalar");
            for operator in &operators {
                assert!(
                    matches!(
                        world.resolve(operator, &subject),
                        Err(HolonError::UnsupportedOperator { .. })
                    ),
                    "{kind_anchor} acquired ordering"
                );
            }
        }
    }

    #[test]
    fn an_ordered_extension_operator_keeps_its_ancestor_comparison() {
        let world = world();
        let strictly_after = stage_operator(
            &world.context,
            OperatorFixture {
                type_name: "StrictlyAfterOperator",
                category: "Ordering",
                case_matching: None,
                operand_type: None,
                extends: Some(&world.greater_than),
            },
        )
        .unwrap();
        let count = world.subject("IntegerValueType", &[&strictly_after]);
        let resolved = world.resolve(&strictly_after, &count).unwrap();
        assert_eq!(resolved.kind(), OperatorKind::Ordered(OrderedComparison::GreaterThan));
        assert!(resolved.invoke(&integer(2), &[integer(1)]).unwrap());
        assert!(!resolved.invoke(&integer(1), &[integer(1)]).unwrap());
        // Affording the extension does not afford its ancestor.
        assert!(matches!(
            world.resolve(&world.greater_than, &count),
            Err(HolonError::UnsupportedOperator { .. })
        ));
    }

    #[test]
    fn an_unrelated_same_named_ordered_operator_has_no_semantics() {
        let world = world();
        for name in ["GreaterThanOperator", "LessThanOrEqualOperator", "GreaterThanOrEqualOperator"]
        {
            let rogue: HolonReference =
                new_descriptor_holon(&world.context, &format!("rogue-{name}"), name, "Operator")
                    .unwrap()
                    .into();
            assert!(
                matches!(
                    OperatorDescriptor::from_holon(rogue).kind(),
                    Err(HolonError::NotImplemented(_))
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn ordered_metadata_must_declare_arity_minus_one_specifications() {
        let world = world();
        let count = world.subject("IntegerValueType", &[&world.greater_than]);
        let HolonReference::Staged(mut greater_than) = world.greater_than.clone() else {
            panic!("canonical operator fixture is staged");
        };
        greater_than.with_property_value(CorePropertyTypeName::Arity, MapInteger(3)).unwrap();
        assert!(matches!(
            world.resolve(&world.greater_than, &count),
            Err(HolonError::InvalidOperatorMetadata { operator, .. })
                if operator == "GreaterThanOperator"
        ));
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
    fn enum_equals_invocation_reads_no_variant_declarations() {
        let world = world();
        let anchor: HolonReference = world
            .context
            .mutation()
            .stage_new_holon(
                new_kind_descriptor(&world.context, "EnumValueType", None, true).unwrap(),
            )
            .unwrap()
            .into();
        let variants: Vec<HolonReference> = ["Red", "Blue"]
            .iter()
            .map(|name| {
                new_descriptor_holon(&world.context, name, name, "EnumVariant").unwrap().into()
            })
            .collect();
        let mut color = new_kind_descriptor(&world.context, "Color", Some(&anchor), false).unwrap();
        color.add_related_holons(CoreRelationshipTypeName::Variants, variants.clone()).unwrap();
        color
            .add_related_holons(
                CoreRelationshipTypeName::AffordsOperator,
                vec![world.equals.clone()],
            )
            .unwrap();
        let subject = ValueDescriptor::from_holon(color.clone().into());
        let equals = world.resolve(&world.equals, &subject).unwrap();

        // Withdraw every declared variant after resolution. Explicit validation
        // sees the change; invocation does not consult the declarations.
        color.remove_related_holons(CoreRelationshipTypeName::Variants, variants).unwrap();
        let token = |name: &str| BaseValue::EnumValue(MapEnumValue(MapString(name.to_string())));
        assert!(matches!(
            subject.is_valid(&token("Red")),
            Err(HolonError::EnumVariantNotInSchema { .. })
        ));
        assert!(equals.invoke(&token("Red"), &[token("Red")]).unwrap());
        assert!(!equals.invoke(&token("Red"), &[token("Blue")]).unwrap());
        assert!(matches!(
            equals.invoke(&string("Red"), &[token("Red")]),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Enum" && found == "String"
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
