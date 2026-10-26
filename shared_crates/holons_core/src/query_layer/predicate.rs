//! Shared predicate validation and evaluation for QueryCore.
//!
//! `Filter`, `SeedHolons.SeedPredicate` and `Expand.ExpansionPredicate` all use
//! this one evaluator. Evaluation has two phases:
//!
//! - [`validate`] checks a predicate definition against a collection's
//!   declared `ElementType` and returns an execution-local
//!   [`ResolvedPredicate`]. It takes no members, so it never reads a
//!   candidate, and an invalid definition fails even for empty input.
//! - [`filter`] applies the resolved predicate to each occurrence, reading the
//!   selected property through ordinary bound reference access.
//!
//! Supported forms are a `ComparisonPredicate` whose operator is Contains or
//! Equals, and a flat `AnyOf` of such leaves. Every other predicate form,
//! nested `AnyOf`, and comparison through any other operator fail with
//! `NotImplemented`; nothing is ever answered with unfiltered members. Neither
//! phase writes to the definitions.
//!
//! Authored literals are checked for declared-type compatibility, runtime
//! representation, and the existing supported value checks (such as enum
//! membership). A literal ValueType with any effective configured constraint,
//! inherited ones included, is rejected: this module evaluates no configured
//! constraint. Stored member values are never revalidated.

use base_types::BaseValue;
use core_types::{HolonError, PropertyName};
use type_names::{
    value_type_descriptor_key, CoreRelationshipTypeName, CoreValueTypeName, QueryHolonTypeName,
    QueryPropertyTypeName, QueryRelationshipTypeName,
};

use crate::descriptors::accessor_helpers::{
    descriptor_label, related_members, require_single_related,
};
use crate::descriptors::{
    classify_holon_type, equals_or_extends, require_holon_type, resolve_core_descriptor,
    same_definition, Descriptor, HolonDescriptor, OperatorDescriptor, OperatorKind,
    ResolvedOperator, ValueDescriptor,
};
use crate::reference_layer::{HolonReference, ReadableHolon};

/// A predicate validated against one declared element type.
///
/// Execution-local: it borrows nothing from and writes nothing to the
/// definition it was resolved from.
#[derive(Debug, Clone)]
pub(crate) enum ResolvedPredicate {
    Comparison(ResolvedComparison),
    /// A flat disjunction of one or more comparison leaves.
    AnyOf(Vec<ResolvedComparison>),
}

/// One validated `ComparisonPredicate`: the selected property contract, the
/// operator resolved for that property's ValueType, and the authored operand
/// values in position order.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedComparison {
    property_name: PropertyName,
    required: bool,
    operator: ResolvedOperator,
    operands: Vec<BaseValue>,
}

/// Predicate forms this runtime recognizes.
#[derive(Debug, Clone, Copy)]
enum PredicateKind {
    Comparison,
    AnyOf,
}

/// Validates `predicate` against `element_type` without reading any candidate.
///
/// Every `AnyOf` child is validated before anything executes, so an invalid
/// or unsupported branch fails the whole predicate.
pub(crate) fn validate(
    predicate: &HolonReference,
    element_type: &HolonDescriptor,
) -> Result<ResolvedPredicate, HolonError> {
    match classify(predicate)? {
        PredicateKind::Comparison => {
            Ok(ResolvedPredicate::Comparison(validate_comparison(predicate, element_type)?))
        }
        PredicateKind::AnyOf => {
            let children = related_members(predicate, QueryRelationshipTypeName::Predicates)?;
            // Schema cardinality is OneOrMore, but transient definitions are
            // not checked against it.
            if children.is_empty() {
                return Err(HolonError::InvalidParameter(format!(
                    "AnyOf {} requires one or more Predicates",
                    predicate.summarize()?
                )));
            }
            let mut leaves = Vec::with_capacity(children.len());
            for child in &children {
                match classify(child)? {
                    PredicateKind::Comparison => {
                        leaves.push(validate_comparison(child, element_type)?)
                    }
                    // Flat groups only; refusing every nested group also
                    // leaves no path by which a cycle could be reached.
                    PredicateKind::AnyOf => {
                        return Err(HolonError::NotImplemented(
                            "nested AnyOf predicate composition".to_string(),
                        ))
                    }
                }
            }
            Ok(ResolvedPredicate::AnyOf(leaves))
        }
    }
}

/// Retains the occurrences of `members` that `predicate` matches, in input
/// order with duplicate occurrences intact.
pub(crate) fn filter(
    members: &[HolonReference],
    predicate: &ResolvedPredicate,
) -> Result<Vec<HolonReference>, HolonError> {
    let mut retained = Vec::new();
    for member in members {
        if predicate.matches(member)? {
            retained.push(member.clone());
        }
    }
    Ok(retained)
}

impl ResolvedPredicate {
    /// Evaluates this predicate against one candidate.
    ///
    /// `AnyOf` evaluates every leaf before combining them, so a true branch
    /// never hides another branch's read, presence, or representation error.
    pub(crate) fn matches(&self, member: &HolonReference) -> Result<bool, HolonError> {
        match self {
            Self::Comparison(leaf) => leaf.matches(member),
            Self::AnyOf(leaves) => {
                let mut matched = false;
                for leaf in leaves {
                    matched |= leaf.matches(member)?;
                }
                Ok(matched)
            }
        }
    }
}

impl ResolvedComparison {
    /// An absent optional property is false; an absent required property is
    /// an execution error. A present value is applied to the operator, which
    /// checks its runtime representation.
    fn matches(&self, member: &HolonReference) -> Result<bool, HolonError> {
        match member.property_value(&self.property_name)? {
            Some(value) => self.operator.invoke(&value, &self.operands),
            None if self.required => Err(HolonError::EmptyField(self.property_name.to_string())),
            None => Ok(false),
        }
    }
}

/// Classifies `predicate` by the nearest supported predicate form in its
/// descriptor lineage, so an extension of a supported form is evaluated as it.
fn classify(predicate: &HolonReference) -> Result<PredicateKind, HolonError> {
    let supported = [
        (QueryHolonTypeName::ComparisonPredicate, PredicateKind::Comparison),
        (QueryHolonTypeName::AnyOf, PredicateKind::AnyOf),
    ]
    .map(|(type_name, kind)| (type_name.as_holon_name(), kind));
    match classify_holon_type(predicate, &supported)? {
        Some(kind) => Ok(kind),
        None => Err(HolonError::NotImplemented(format!(
            "QueryPredicate evaluation: {}",
            predicate.holon_descriptor()?.header().type_name()?
        ))),
    }
}

/// Validates one comparison leaf: its shape, its selector on the declared
/// type, its operator against the selected property's ValueType, and each
/// authored operand against the type the operator requires at its position.
fn validate_comparison(
    predicate: &HolonReference,
    element_type: &HolonDescriptor,
) -> Result<ResolvedComparison, HolonError> {
    let property_name = read_property_name(predicate)?;
    let operator = OperatorDescriptor::from_holon(require_single_related(
        predicate,
        QueryRelationshipTypeName::Operator,
    )?);
    let operands = related_members(predicate, QueryRelationshipTypeName::Operands)?;

    // Selectors resolve on the declared type only, inherited declarations
    // included; a member subtype's own declarations are not selectable.
    let property = element_type.get_property_by_name(property_name.clone())?;
    let required = property.is_required()?;
    let subject_type = property.value_type()?;

    let operator = operator.resolve_for_subject(&subject_type)?;
    match operator.kind() {
        OperatorKind::Contains(_) => require_string_subject(&operator)?,
        // Equals' operand is the subject's own ValueType, so resolution
        // already carries its whole contract.
        OperatorKind::Equals => {}
        OperatorKind::LessThan => {
            return Err(HolonError::NotImplemented(format!(
                "comparison predicates through {}",
                operator.operator().operator_name()?
            )))
        }
    }

    let operand_values = validate_operands(&operator, &operands)?;
    Ok(ResolvedComparison { property_name, required, operator, operands: operand_values })
}

/// Reads the required, default-less `PropertyName` selector. It must be a
/// string; there is no fallback.
fn read_property_name(predicate: &HolonReference) -> Result<PropertyName, HolonError> {
    match predicate.property_value(QueryPropertyTypeName::PropertyName)? {
        Some(BaseValue::StringValue(name)) => Ok(PropertyName(name)),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{other:?}"), "String".to_string()))
        }
        None => Err(HolonError::EmptyField(
            QueryPropertyTypeName::PropertyName.as_property_name().to_string(),
        )),
    }
}

/// Contains requires a subject ValueType in the `MapStringValueType` lineage,
/// by descriptor identity. This is stricter than the string value-kind check
/// that operator resolution applies.
fn require_string_subject(operator: &ResolvedOperator) -> Result<(), HolonError> {
    let subject_type = operator.subject_type();
    let string_type = resolve_core_descriptor(
        &subject_type.holon().resolution_context()?,
        &value_type_descriptor_key(
            &CoreValueTypeName::MapStringValueType.as_value_name(),
            &CoreValueTypeName::StringValueType,
        )
        .0,
    )?;
    if equals_or_extends(subject_type.holon(), &string_type)? {
        return Ok(());
    }
    Err(HolonError::UnsupportedOperator {
        operator: operator.operator().operator_name()?.to_string(),
        value_type: subject_type.header().type_name()?.to_string(),
        descriptor: descriptor_label(subject_type.holon()),
    })
}

/// Checks the authored operands against the resolved operator's ordered
/// specifications and returns their values in position order.
fn validate_operands(
    operator: &ResolvedOperator,
    operands: &[HolonReference],
) -> Result<Vec<BaseValue>, HolonError> {
    let required_types = operator.operand_value_types();
    if operands.len() != required_types.len() {
        return Err(HolonError::OperandCountMismatch {
            operator: operator.operator().operator_name()?.to_string(),
            expected: required_types.len(),
            found: operands.len(),
        });
    }
    operands
        .iter()
        .zip(required_types)
        .map(|(operand, required_type)| validate_literal_operand(operator, operand, required_type))
        .collect()
}

/// Validates one `LiteralOperand`: its declared type D must equal or extend
/// the required type R by identity, and its value must pass the authored
/// literal checks of both D and R (once when they are the same descriptor).
fn validate_literal_operand(
    operator: &ResolvedOperator,
    operand: &HolonReference,
    required_type: &ValueDescriptor,
) -> Result<BaseValue, HolonError> {
    require_holon_type(operand, &QueryHolonTypeName::LiteralOperand.as_holon_name())?;
    let value = operand.property_value(QueryPropertyTypeName::OperandValue)?.ok_or_else(|| {
        HolonError::EmptyField(QueryPropertyTypeName::OperandValue.as_property_name().to_string())
    })?;
    // The literal's ValueType relationship shares Core's `ValueType` name.
    let declared_type = ValueDescriptor::from_holon(require_single_related(
        operand,
        CoreRelationshipTypeName::ValueType,
    )?);

    if !equals_or_extends(declared_type.holon(), required_type.holon())? {
        return Err(HolonError::IncompatibleLiteralType {
            declared: descriptor_label(declared_type.holon()),
            required: descriptor_label(required_type.holon()),
            operator: operator.operator().operator_name()?.to_string(),
        });
    }
    check_authored_literal(&declared_type, &value)?;
    if !same_definition(declared_type.holon(), required_type.holon())? {
        check_authored_literal(required_type, &value)?;
    }
    Ok(value)
}

/// The authored-literal checks available without configured-constraint
/// evaluators: runtime representation and the existing value checks, such as
/// enum membership, through `is_valid`. Any effective configured constraint,
/// inherited ones included, fails explicitly rather than passing unchecked.
///
/// This is the single point to switch to the shared configured-value
/// validation facility once its constraint evaluators exist.
fn check_authored_literal(
    value_type: &ValueDescriptor,
    value: &BaseValue,
) -> Result<(), HolonError> {
    value_type.is_valid(value)?;
    if let Some(constraint) = value_type.effective_constraints()?.first() {
        return Err(HolonError::UnsupportedValueConstraint {
            constraint_type: constraint
                .member
                .holon_descriptor()?
                .header()
                .type_name()?
                .to_string(),
            value_type: descriptor_label(value_type.holon()),
            descriptor: descriptor_label(&constraint.declared_on),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_holon_type_descriptor};
    use crate::query_layer::predicate_test_support::{
        bytes, canonical_type, flag, number, text, token, PredicateWorld,
    };
    use crate::reference_layer::WritableHolon;
    use type_names::QueryHolonTypeName as Q;

    fn world() -> PredicateWorld {
        PredicateWorld::new(build_context())
    }

    fn book(world: &PredicateWorld) -> HolonDescriptor {
        PredicateWorld::descriptor(&world.book)
    }

    fn matches_of(
        predicate: &ResolvedPredicate,
        members: &[HolonReference],
    ) -> Result<Vec<bool>, HolonError> {
        members.iter().map(|member| predicate.matches(member)).collect()
    }

    // ---- Selection and capability ------------------------------------------

    #[test]
    fn contains_folds_case_on_a_required_string_property() {
        let world = world();
        let predicate = validate(&world.contains("Title", "STRASSE"), &book(&world)).unwrap();
        let members = [
            world.member("a", &world.book, &[("Title", text("Hauptstraße 5"))]),
            world.member("b", &world.book, &[("Title", text("Main Street"))]),
        ];
        assert_eq!(matches_of(&predicate, &members).unwrap(), vec![true, false]);
    }

    #[test]
    fn validation_reads_no_candidate() {
        // `validate` takes no members: a definition error surfaces with no
        // collection in sight.
        let world = world();
        let error = validate(&world.contains("Missing", "x"), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { name, .. } if name == "Missing"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_inherited_selector_is_accepted_and_a_subtype_only_selector_rejected() {
        let world = world();
        let novel = PredicateWorld::descriptor(&world.novel);
        validate(&world.contains("Title", "x"), &novel).expect("Title is inherited from Book");
        validate(&world.contains("Narrator", "x"), &novel).expect("Narrator is declared on Novel");

        // Every member may be a Novel, but the declared type decides.
        let error = validate(&world.contains("Narrator", "x"), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { name, .. } if name == "Narrator"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn ordering_predicates_are_not_implemented() {
        let world = world();
        let comparison = world.comparison(
            "Title",
            &world.less_than,
            vec![world.literal(&world.text, text("x"))],
        );
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(matches!(error, HolonError::NotImplemented(_)), "unexpected error: {error:?}");
    }

    #[test]
    fn contains_requires_a_subject_in_the_map_string_lineage() {
        let world = world();
        // `Code` is string-kind and affords Contains, but its type does not
        // extend MapStringValueType.
        let error = validate(&world.contains("Code", "x"), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedOperator { value_type, .. } if value_type == "ForeignString"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_unafforded_operator_is_unsupported() {
        let world = world();
        let error = validate(&world.contains("PageCount", "1"), &book(&world)).unwrap_err();
        assert!(
            matches!(error, HolonError::UnsupportedOperator { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn property_name_must_be_a_present_string() {
        let world = world();
        let mut missing = world.described("no-name", Q::ComparisonPredicate);
        missing
            .add_related_holons(QueryRelationshipTypeName::Operator, vec![world.contains.clone()])
            .unwrap();
        let error = validate(&missing.into(), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::EmptyField(name) if name == "PropertyName"),
            "unexpected error: {error:?}"
        );

        let mut numeric = world.described("numeric-name", Q::ComparisonPredicate);
        numeric.with_property_value(QueryPropertyTypeName::PropertyName, number(1)).unwrap();
        let error = validate(&numeric.into(), &book(&world)).unwrap_err();
        assert!(
            matches!(error, HolonError::UnexpectedValueType(..)),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn a_comparison_requires_exactly_one_operator() {
        let world = world();
        let mut predicate = world.described("no-operator", Q::ComparisonPredicate);
        predicate.with_property_value(QueryPropertyTypeName::PropertyName, "Title").unwrap();
        let error = validate(&predicate.into(), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::MissingRequiredRelationship { relationship, .. } if relationship == "Operator"),
            "unexpected error: {error:?}"
        );
    }

    // ---- Authored literals --------------------------------------------------

    #[test]
    fn the_operand_count_must_match_the_operator() {
        let world = world();
        for operands in [vec![], vec![world.substring("a"), world.substring("b")]] {
            let found = operands.len();
            let comparison = world.comparison("Title", &world.contains, operands);
            let error = validate(&comparison, &book(&world)).unwrap_err();
            assert!(
                matches!(error, HolonError::OperandCountMismatch { expected: 1, found: f, .. } if f == found),
                "unexpected error for {found} operands: {error:?}"
            );
        }
    }

    #[test]
    fn a_literal_declared_outside_the_required_type_is_incompatible() {
        let world = world();
        let comparison = world.comparison(
            "Title",
            &world.contains,
            vec![world.literal(&world.integer, number(5))],
        );
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(
            matches!(error, HolonError::IncompatibleLiteralType { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn a_literal_whose_value_does_not_match_its_type_is_rejected() {
        let world = world();
        let comparison = world.comparison(
            "Title",
            &world.contains,
            vec![world.literal(&world.map_string, number(5))],
        );
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(
            matches!(error, HolonError::ValueKindMismatch { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_operand_must_be_a_literal_operand_with_a_value() {
        let world = world();
        let not_a_literal: HolonReference =
            world.described("not-a-literal", Q::ComparisonPredicate).into();
        let comparison = world.comparison("Title", &world.contains, vec![not_a_literal]);
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::WrongDescriptorKind { expected, .. } if expected == "LiteralOperand"),
            "unexpected error: {error:?}"
        );

        let mut valueless = world.described("valueless", Q::LiteralOperand);
        valueless
            .add_related_holons(CoreRelationshipTypeName::ValueType, vec![world.map_string.clone()])
            .unwrap();
        let comparison = world.comparison("Title", &world.contains, vec![valueless.into()]);
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::EmptyField(name) if name == "OperandValue"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn a_constrained_subject_accepts_an_unconstrained_substring() {
        // The substring is checked against its own type and the operator's
        // required type, never against the subject's constraint.
        let world = world();
        let predicate = validate(&world.contains("Identifier", "graph"), &book(&world)).unwrap();
        let member = world.member("a", &world.book, &[("Identifier", text("map-graph-7"))]);
        assert!(predicate.matches(&member).unwrap());
    }

    #[test]
    fn a_constrained_literal_type_fails_explicitly() {
        let world = world();
        // Identifier extends MapStringValueType through Text, so it satisfies
        // Contains' required type, but it carries a constraint.
        let comparison = world.comparison(
            "Title",
            &world.contains,
            vec![world.literal(&world.identifier, text("graph"))],
        );
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedValueConstraint { constraint_type, .. }
                if constraint_type == "StringPatternConstraint"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_inherited_literal_constraint_fails_explicitly() {
        let world = world();
        // No local constraint: the one on Identifier is inherited.
        let short_identifier: HolonReference =
            crate::descriptors::test_support::new_kind_descriptor(
                &world.context,
                "ShortIdentifier",
                Some(&world.identifier),
                false,
            )
            .unwrap()
            .into();
        let comparison = world.comparison(
            "Title",
            &world.contains,
            vec![world.literal(&short_identifier, text("graph"))],
        );
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedValueConstraint { value_type, descriptor, .. }
                if value_type.contains("ShortIdentifier") && descriptor.contains("Identifier")
                    && !descriptor.contains("Short")),
            "the error names the literal type and the declaring ancestor: {error:?}"
        );
    }

    #[test]
    fn a_constrained_required_type_fails_every_contains_literal() {
        // Fail closed: once the canonical string type carries a constraint,
        // no Contains literal passes until that constraint can be evaluated.
        let world = PredicateWorld::with_constrained_map_string(build_context());
        let error = validate(&world.contains("Title", "x"), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedValueConstraint { constraint_type, .. }
                if constraint_type == "StringLengthConstraint"),
            "unexpected error: {error:?}"
        );
    }

    // ---- Evaluation ---------------------------------------------------------

    #[test]
    fn optional_absence_is_false_and_required_absence_fails() {
        let world = world();
        let untitled = world.member("untitled", &world.book, &[]);

        let subtitle = validate(&world.contains("Subtitle", ""), &book(&world)).unwrap();
        assert!(!subtitle.matches(&untitled).unwrap(), "even the empty substring needs a value");

        let title = validate(&world.contains("Title", ""), &book(&world)).unwrap();
        let error = title.matches(&untitled).unwrap_err();
        assert!(
            matches!(&error, HolonError::EmptyField(name) if name == "Title"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn a_stored_value_of_the_wrong_representation_fails() {
        let world = world();
        let predicate = validate(&world.contains("Subtitle", "x"), &book(&world)).unwrap();
        let member = world.member("a", &world.book, &[("Subtitle", number(3))]);
        assert!(matches!(predicate.matches(&member), Err(HolonError::ValueKindMismatch { .. })));
    }

    #[test]
    fn filter_keeps_order_and_duplicate_occurrences() {
        let world = world();
        let predicate = validate(&world.contains("Title", "map"), &book(&world)).unwrap();
        let a = world.member("a", &world.book, &[("Title", text("Map A"))]);
        let b = world.member("b", &world.book, &[("Title", text("Other"))]);
        let c = world.member("c", &world.book, &[("Title", text("Roadmap"))]);
        let input = vec![c.clone(), a.clone(), b, a.clone()];
        assert_eq!(filter(&input, &predicate).unwrap(), vec![c, a.clone(), a]);
    }

    // ---- AnyOf --------------------------------------------------------------

    #[test]
    fn any_of_keeps_an_occurrence_matching_several_leaves_once() {
        let world = world();
        let predicate = validate(
            &world.any_of(vec![world.contains("Title", "map"), world.contains("Subtitle", "map")]),
            &book(&world),
        )
        .unwrap();
        let both = world.member(
            "both",
            &world.book,
            &[("Title", text("Map")), ("Subtitle", text("A map"))],
        );
        let subtitle_only = world.member(
            "subtitle-only",
            &world.book,
            &[("Title", text("Atlas")), ("Subtitle", text("Maps"))],
        );
        let neither = world.member("neither", &world.book, &[("Title", text("Atlas"))]);
        assert_eq!(
            filter(&[both.clone(), subtitle_only.clone(), neither], &predicate).unwrap(),
            vec![both, subtitle_only]
        );
    }

    #[test]
    fn a_true_branch_does_not_hide_a_later_error() {
        let world = world();
        let predicate = validate(
            &world.any_of(vec![world.contains("Subtitle", "map"), world.contains("Title", "map")]),
            &book(&world),
        )
        .unwrap();
        // The first leaf matches; the second must still read the required Title.
        let member = world.member("untitled", &world.book, &[("Subtitle", text("map"))]);
        assert!(matches!(predicate.matches(&member), Err(HolonError::EmptyField(_))));
    }

    #[test]
    fn every_branch_is_validated_before_anything_executes() {
        let world = world();
        let error = validate(
            &world.any_of(vec![world.contains("Title", "x"), world.contains("Missing", "x")]),
            &book(&world),
        )
        .unwrap_err();
        assert!(
            matches!(error, HolonError::DescriptorDeclarationNotFound { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_empty_any_of_is_invalid() {
        let world = world();
        let error = validate(&world.any_of(vec![]), &book(&world)).unwrap_err();
        assert!(matches!(error, HolonError::InvalidParameter(_)), "unexpected error: {error:?}");
    }

    #[test]
    fn nested_and_unsupported_children_are_not_implemented() {
        let world = world();
        let nested = world.any_of(vec![
            world.contains("Title", "x"),
            world.any_of(vec![world.contains("Title", "y")]),
        ]);
        let error = validate(&nested, &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::NotImplemented(detail) if detail.contains("nested AnyOf")),
            "unexpected error: {error:?}"
        );

        let not = canonical_type(&world.context, "Not");
        let mut negation =
            crate::descriptors::test_support::new_test_holon(&world.context, "not").unwrap();
        negation.with_descriptor(not).unwrap();
        let error = validate(&world.any_of(vec![negation.into()]), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::NotImplemented(detail) if detail.contains("Not")),
            "unexpected error: {error:?}"
        );
    }

    // ---- Equals -------------------------------------------------------------

    #[test]
    fn equals_is_case_sensitive_and_independent_of_contains_folding() {
        let world = world();
        let predicate =
            validate(&world.equals("Title", &world.text, text("Straße")), &book(&world)).unwrap();
        let members = [
            world.member("exact", &world.book, &[("Title", text("Straße"))]),
            world.member("folded", &world.book, &[("Title", text("STRASSE"))]),
            world.member("lower", &world.book, &[("Title", text("straße"))]),
        ];
        assert_eq!(matches_of(&predicate, &members).unwrap(), vec![true, false, false]);
    }

    #[test]
    fn equals_matches_each_afforded_scalar_domain() {
        let world = world();
        let cases = [
            ("PageCount", &world.integer, number(3), number(4)),
            ("IsPublished", &world.flag, flag(true), flag(false)),
            ("Status", &world.status, token("Published"), token("Draft")),
            ("Digest", &world.digest, bytes(&[1, 2]), bytes(&[2, 1])),
        ];
        for (property, value_type, wanted, other) in cases {
            let predicate =
                validate(&world.equals(property, value_type, wanted.clone()), &book(&world))
                    .unwrap();
            let members = [
                world.member("match", &world.book, &[("Title", text("t")), (property, wanted)]),
                world.member("miss", &world.book, &[("Title", text("t")), (property, other)]),
            ];
            assert_eq!(matches_of(&predicate, &members).unwrap(), vec![true, false], "{property}");
        }
    }

    #[test]
    fn equals_on_an_unafforded_bytes_type_is_unsupported() {
        // Bytes equality executes, but only an explicit affordance admits it.
        let world = world();
        let comparison = world.equals("RawDigest", &world.raw_digest, bytes(&[1]));
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedOperator { value_type, .. } if value_type == "RawDigest"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_inherited_equals_affordance_is_accepted() {
        let world = world();
        let predicate =
            validate(&world.equals("Headline", &world.headline, text("News")), &book(&world))
                .unwrap();
        let member = world.member("a", &world.book, &[("Headline", text("News"))]);
        assert!(predicate.matches(&member).unwrap());
    }

    #[test]
    fn an_equals_literal_may_declare_a_subtype_of_the_subject_type() {
        let world = world();
        let predicate =
            validate(&world.equals("Title", &world.headline, text("Atlas")), &book(&world))
                .unwrap();
        let member = world.member("a", &world.book, &[("Title", text("Atlas"))]);
        assert!(predicate.matches(&member).unwrap());
    }

    #[test]
    fn an_equals_literal_declared_outside_the_subject_type_is_incompatible() {
        // MapStringValueType shares Text's representation but is broader than it.
        let world = world();
        for (value_type, value) in [(&world.map_string, text("Atlas")), (&world.integer, number(1))]
        {
            let error =
                validate(&world.equals("Title", value_type, value), &book(&world)).unwrap_err();
            assert!(
                matches!(error, HolonError::IncompatibleLiteralType { .. }),
                "unexpected error: {error:?}"
            );
        }
    }

    #[test]
    fn an_undeclared_enum_literal_fails_validation() {
        let world = world();
        let error =
            validate(&world.equals("Status", &world.status, token("Archived")), &book(&world))
                .unwrap_err();
        assert!(
            matches!(&error, HolonError::EnumVariantNotInSchema { variant, .. } if variant == "Archived"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_undeclared_stored_enum_token_is_unequal_not_an_error() {
        // Stored values are never revalidated; only their representation matters.
        let world = world();
        let predicate =
            validate(&world.equals("Status", &world.status, token("Draft")), &book(&world))
                .unwrap();
        let undeclared = world.member("a", &world.book, &[("Status", token("Archived"))]);
        assert!(!predicate.matches(&undeclared).unwrap());

        let not_an_enum = world.member("b", &world.book, &[("Status", number(3))]);
        assert!(matches!(
            predicate.matches(&not_an_enum),
            Err(HolonError::ValueKindMismatch { .. })
        ));
    }

    #[test]
    fn equals_takes_exactly_one_operand() {
        let world = world();
        let literal = |value: &str| world.literal(&world.text, text(value));
        for operands in [vec![], vec![literal("a"), literal("b")]] {
            let found = operands.len();
            let comparison = world.comparison("Title", &world.equals, operands);
            let error = validate(&comparison, &book(&world)).unwrap_err();
            assert!(
                matches!(error, HolonError::OperandCountMismatch { expected: 1, found: f, .. } if f == found),
                "unexpected error for {found} operands: {error:?}"
            );
        }
    }

    #[test]
    fn equals_on_a_constrained_subject_fails_explicitly() {
        // Unlike a Contains substring, an Equals literal must satisfy the
        // subject's own ValueType, constraints included.
        let world = world();
        let comparison = world.equals("Identifier", &world.identifier, text("map-graph-7"));
        let error = validate(&comparison, &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedValueConstraint { constraint_type, .. }
                if constraint_type == "StringPatternConstraint"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn a_constrained_canonical_string_fails_equals_on_its_subtypes() {
        // Fail closed: Text inherits the canonical constraint, so no Equals
        // literal on it passes until that constraint can be evaluated.
        let world = PredicateWorld::with_constrained_map_string(build_context());
        let error =
            validate(&world.equals("Title", &world.text, text("x")), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedValueConstraint { constraint_type, .. }
                if constraint_type == "StringLengthConstraint"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn equals_optional_absence_is_false_and_required_absence_fails() {
        let world = world();
        let untitled = world.member("untitled", &world.book, &[]);

        let status =
            validate(&world.equals("Status", &world.status, token("Draft")), &book(&world))
                .unwrap();
        assert!(!status.matches(&untitled).unwrap());

        let title = validate(&world.equals("Title", &world.text, text("")), &book(&world)).unwrap();
        assert!(matches!(
            title.matches(&untitled),
            Err(HolonError::EmptyField(name)) if name == "Title"
        ));
    }

    #[test]
    fn a_mixed_any_of_keeps_an_occurrence_matching_both_leaves_once() {
        let world = world();
        let predicate = validate(
            &world.any_of(vec![
                world.equals("Title", &world.text, text("Atlas")),
                world.contains("Subtitle", "map"),
            ]),
            &book(&world),
        )
        .unwrap();
        let both = world.member(
            "both",
            &world.book,
            &[("Title", text("Atlas")), ("Subtitle", text("A map"))],
        );
        let equals_only = world.member("equals-only", &world.book, &[("Title", text("Atlas"))]);
        let contains_only = world.member(
            "contains-only",
            &world.book,
            &[("Title", text("Roads")), ("Subtitle", text("Maps"))],
        );
        let neither = world.member("neither", &world.book, &[("Title", text("atlas"))]);
        assert_eq!(
            filter(
                &[both.clone(), equals_only.clone(), neither, contains_only.clone(), both.clone()],
                &predicate
            )
            .unwrap(),
            vec![both.clone(), equals_only, contains_only, both]
        );
    }

    #[test]
    fn a_true_equals_branch_does_not_hide_a_later_error() {
        let world = world();
        let predicate = validate(
            &world.any_of(vec![
                world.equals("PageCount", &world.integer, number(3)),
                world.equals("Title", &world.text, text("x")),
            ]),
            &book(&world),
        )
        .unwrap();
        let member = world.member("untitled", &world.book, &[("PageCount", number(3))]);
        assert!(matches!(predicate.matches(&member), Err(HolonError::EmptyField(_))));
    }

    #[test]
    fn an_equality_only_any_of_validates_every_branch() {
        let world = world();
        let error = validate(
            &world.any_of(vec![
                world.equals("Status", &world.status, token("Draft")),
                world.equals("Status", &world.status, token("Archived")),
            ]),
            &book(&world),
        )
        .unwrap_err();
        assert!(
            matches!(error, HolonError::EnumVariantNotInSchema { .. }),
            "unexpected error: {error:?}"
        );
    }

    // ---- Extension types ----------------------------------------------------

    #[test]
    fn an_extension_of_comparison_predicate_is_evaluated_as_one() {
        let world = world();
        let mut extension_type =
            new_holon_type_descriptor(&world.context, "TitleSearch.HolonType", "TitleSearch")
                .unwrap();
        extension_type
            .add_related_holons(
                CoreRelationshipTypeName::Extends,
                vec![canonical_type(&world.context, "ComparisonPredicate")],
            )
            .unwrap();
        let mut predicate =
            crate::descriptors::test_support::new_test_holon(&world.context, "title-search")
                .unwrap();
        predicate.with_descriptor(extension_type.into()).unwrap();
        predicate.with_property_value(QueryPropertyTypeName::PropertyName, "Title").unwrap();
        predicate
            .add_related_holons(QueryRelationshipTypeName::Operator, vec![world.contains.clone()])
            .unwrap()
            .add_related_holons(QueryRelationshipTypeName::Operands, vec![world.substring("x")])
            .unwrap();
        assert!(matches!(
            validate(&predicate.into(), &book(&world)).unwrap(),
            ResolvedPredicate::Comparison(_)
        ));
    }

    #[test]
    fn a_same_named_unrelated_predicate_type_is_not_implemented() {
        let world = world();
        let impostor = new_holon_type_descriptor(
            &world.context,
            "Impostor.ComparisonPredicate",
            "ComparisonPredicate",
        )
        .unwrap();
        let mut predicate =
            crate::descriptors::test_support::new_test_holon(&world.context, "impostor").unwrap();
        predicate.with_descriptor(impostor.into()).unwrap();
        let error = validate(&predicate.into(), &book(&world)).unwrap_err();
        assert!(matches!(error, HolonError::NotImplemented(_)), "unexpected error: {error:?}");
    }
}
