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
//! Supported forms are a `ComparisonPredicate` whose operator is Contains,
//! Equals, or one of the ordered comparisons (LessThan, GreaterThan,
//! LessThanOrEqual, GreaterThanOrEqual), and the composites `Not`, `AllOf` and
//! `AnyOf`, nested to any depth. Any other predicate form, and comparison
//! through an operator with no supported core semantics in its lineage, fails
//! with `NotImplemented` wherever it appears. Malformed child cardinalities,
//! children that are not predicates, and cycles are definition errors. Nothing
//! is ever answered with unfiltered members, and neither phase writes to the
//! definitions.
//!
//! Authored literals are checked for declared-type compatibility, runtime
//! representation, existing value semantics and effective configured constraints
//! through `ValueDescriptor::validate_literal`. Stored member values are never revalidated.

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
    classify_holon_type, lineage_contains, require_holon_type, same_definition, Descriptor,
    HolonDescriptor, OperatorDescriptor, OperatorKind, ResolvedOperator, ValueDescriptor,
};
use crate::reference_layer::{HolonReference, ReadableHolon};

/// A predicate validated against one declared element type.
///
/// Execution-local: it borrows nothing from and writes nothing to the
/// definition it was resolved from.
#[derive(Debug, Clone)]
pub(crate) enum ResolvedPredicate {
    Comparison(ResolvedComparison),
    Not(Box<ResolvedPredicate>),
    /// A conjunction of one or more predicates, in authored order.
    AllOf(Vec<ResolvedPredicate>),
    /// A disjunction of one or more predicates, in authored order.
    AnyOf(Vec<ResolvedPredicate>),
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
    Composite(Composite),
}

/// The Boolean composites, each over its child predicates.
#[derive(Debug, Clone, Copy)]
enum Composite {
    Not,
    AllOf,
    AnyOf,
}

/// Validates `predicate` against `element_type` without reading any candidate.
///
/// Every reachable child is validated, depth-first in authored order, before
/// anything executes, so an invalid or unsupported branch fails the whole
/// predicate even when another branch would decide its Boolean result.
pub(crate) fn validate(
    predicate: &HolonReference,
    element_type: &HolonDescriptor,
) -> Result<ResolvedPredicate, HolonError> {
    resolve(predicate, element_type, &mut Vec::new())
}

/// Resolves `predicate`, whose ancestors from the root (exclusive) are
/// `ancestors`.
fn resolve(
    predicate: &HolonReference,
    element_type: &HolonDescriptor,
    ancestors: &mut Vec<HolonReference>,
) -> Result<ResolvedPredicate, HolonError> {
    let composite = match classify(predicate)? {
        PredicateKind::Comparison => {
            return Ok(ResolvedPredicate::Comparison(validate_comparison(predicate, element_type)?))
        }
        PredicateKind::Composite(composite) => composite,
    };
    let (relationship, children) = match composite {
        Composite::Not => (
            QueryRelationshipTypeName::Predicate,
            vec![require_single_related(predicate, QueryRelationshipTypeName::Predicate)?],
        ),
        Composite::AllOf | Composite::AnyOf => {
            let children = related_members(predicate, QueryRelationshipTypeName::Predicates)?;
            // Schema cardinality is OneOrMore, but transient definitions are
            // not checked against it.
            if children.is_empty() {
                return Err(HolonError::InvalidParameter(format!(
                    "{composite:?} {} requires one or more Predicates",
                    predicate.summarize()?
                )));
            }
            (QueryRelationshipTypeName::Predicates, children)
        }
    };

    ancestors.push(predicate.clone());
    let resolved = children
        .iter()
        .map(|child| resolve_child(predicate, &relationship, child, element_type, ancestors))
        .collect::<Result<Vec<_>, _>>();
    ancestors.pop();
    let mut resolved = resolved?;

    Ok(match composite {
        Composite::Not => ResolvedPredicate::Not(Box::new(resolved.remove(0))),
        Composite::AllOf => ResolvedPredicate::AllOf(resolved),
        Composite::AnyOf => ResolvedPredicate::AnyOf(resolved),
    })
}

/// Resolves one child of `parent`, whose path from the root (inclusive) is
/// `ancestors`.
///
/// A cycle is a child that is the same definition as one of its own
/// ancestors. One definition reused in separate branches, or repeated within
/// one group, is not a cycle and is resolved at each occurrence.
fn resolve_child(
    parent: &HolonReference,
    relationship: &QueryRelationshipTypeName,
    child: &HolonReference,
    element_type: &HolonDescriptor,
    ancestors: &mut Vec<HolonReference>,
) -> Result<ResolvedPredicate, HolonError> {
    require_holon_type(child, &QueryHolonTypeName::QueryPredicate.as_holon_name())?;
    if lineage_contains(ancestors.iter(), child)? {
        return Err(HolonError::InvalidParameter(format!(
            "QueryPredicate {} revisits {} through {}; the predicate graph is cyclic",
            parent.summarize()?,
            child.summarize()?,
            relationship.as_relationship_name()
        )));
    }
    resolve(child, element_type, ancestors)
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
    /// Groups evaluate every child before combining them, with no
    /// short-circuit, so neither a true `AnyOf` branch nor a false `AllOf`
    /// branch hides another branch's read, presence, or representation error.
    pub(crate) fn matches(&self, member: &HolonReference) -> Result<bool, HolonError> {
        match self {
            Self::Comparison(leaf) => leaf.matches(member),
            Self::Not(child) => Ok(!child.matches(member)?),
            Self::AllOf(children) => {
                let mut matched = true;
                for child in children {
                    matched &= child.matches(member)?;
                }
                Ok(matched)
            }
            Self::AnyOf(children) => {
                let mut matched = false;
                for child in children {
                    matched |= child.matches(member)?;
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
        (QueryHolonTypeName::Not, PredicateKind::Composite(Composite::Not)),
        (QueryHolonTypeName::AllOf, PredicateKind::Composite(Composite::AllOf)),
        (QueryHolonTypeName::AnyOf, PredicateKind::Composite(Composite::AnyOf)),
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
    let property =
        crate::descriptors::EffectiveHolonDescriptor::acquire(element_type.holon().clone())?
            .get_property_by_name(property_name.clone())?;
    let required = property.is_required()?;
    let subject_type = property.value_type()?;

    let operator = operator.resolve_for_subject(&subject_type)?;
    match operator.kind() {
        OperatorKind::Contains(_) => require_string_subject(&operator)?,
        // Equals and the ordered comparisons take the subject's own ValueType
        // as their operand, so resolution already carries their whole
        // contract, including the integer/string ordering domain.
        OperatorKind::Equals | OperatorKind::Ordered(_) => {}
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
    let string_type = crate::descriptors::resolve_core_descriptor_for(
        subject_type.holon(),
        &value_type_descriptor_key(
            &CoreValueTypeName::MapStringValueType.as_value_name(),
            &CoreValueTypeName::StringValueType,
        )
        .0,
    )?;
    if subject_type.equals_or_extends(&string_type)? {
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

    if !declared_type.equals_or_extends(required_type.holon())? {
        return Err(HolonError::IncompatibleLiteralType {
            declared: descriptor_label(declared_type.holon()),
            required: descriptor_label(required_type.holon()),
            operator: operator.operator().operator_name()?.to_string(),
        });
    }
    // D extends R, so D's effective constraints already include R's contributions and are
    // evaluated once. Value semantics such as enum membership are per type, so R still
    // checks the literal's representation and membership itself.
    declared_type.validate_literal(&value)?;
    if !same_definition(declared_type.holon(), required_type.holon())? {
        required_type.is_valid(&value)?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, new_holon_type_descriptor, stage_operator, OperatorFixture,
    };
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
    fn an_afforded_operator_without_supported_core_semantics_is_not_implemented() {
        // Affordance admits the operator, but no supported core operator
        // appears in its lineage, so it has nothing to execute.
        let world = world();
        let unclassified = stage_operator(
            &world.context,
            OperatorFixture {
                type_name: "RoughlyEqualsOperator",
                category: "Equality",
                case_matching: None,
                operand_type: None,
                extends: None,
            },
        )
        .unwrap();
        let HolonReference::Transient(mut text_type) = world.text.clone() else {
            panic!("the Text value type is transient");
        };
        text_type
            .add_related_holons(
                CoreRelationshipTypeName::AffordsOperator,
                vec![unclassified.clone()],
            )
            .unwrap();
        let comparison = world.compare("Title", &unclassified, &world.text, text("x"));
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
    fn an_unsupported_literal_constraint_fails_explicitly() {
        let world = world();
        // Identifier extends MapStringValueType through Text, so it satisfies
        // Contains' required type, but its pattern constraint has no evaluator.
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
    fn an_inherited_unsupported_literal_constraint_fails_explicitly() {
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
    fn a_constrained_required_type_accepts_ordinary_and_empty_contains_literals() {
        let world = PredicateWorld::with_constrained_map_string(build_context());
        for value in ["x", ""] {
            validate(&world.contains("Title", value), &book(&world)).unwrap();
        }
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
    fn an_unsupported_predicate_form_is_not_implemented_at_any_depth() {
        let world = world();
        let mut exists_type =
            new_holon_type_descriptor(&world.context, "Exists.HolonType", "Exists").unwrap();
        exists_type
            .add_related_holons(
                CoreRelationshipTypeName::Extends,
                vec![canonical_type(&world.context, "QueryPredicate")],
            )
            .unwrap();
        let mut exists =
            crate::descriptors::test_support::new_test_holon(&world.context, "exists").unwrap();
        exists.with_descriptor(exists_type.into()).unwrap();
        let exists: HolonReference = exists.into();

        for predicate in [
            exists.clone(),
            world.any_of(vec![world.contains("Title", "x"), exists.clone()]),
            world.all_of(vec![world.not(vec![world.any_of(vec![exists])])]),
        ] {
            let error = validate(&predicate, &book(&world)).unwrap_err();
            assert!(
                matches!(&error, HolonError::NotImplemented(detail) if detail.contains("Exists")),
                "unexpected error: {error:?}"
            );
        }
    }

    // ---- Recursive composition ----------------------------------------------

    /// Books with every combination of a `map` Title and a `map` Subtitle:
    /// [both, title only, subtitle only, neither].
    fn title_subtitle_members(world: &PredicateWorld) -> [HolonReference; 4] {
        [
            world.member("tt", &world.book, &[("Title", text("Map")), ("Subtitle", text("map"))]),
            world.member("tf", &world.book, &[("Title", text("Map")), ("Subtitle", text("x"))]),
            world.member("ft", &world.book, &[("Title", text("x")), ("Subtitle", text("map"))]),
            world.member("ff", &world.book, &[("Title", text("x")), ("Subtitle", text("x"))]),
        ]
    }

    fn evaluate(
        world: &PredicateWorld,
        predicate: &HolonReference,
        members: &[HolonReference],
    ) -> Vec<bool> {
        let predicate = validate(predicate, &book(world)).unwrap();
        matches_of(&predicate, members).unwrap()
    }

    #[test]
    fn not_negates_its_child() {
        let world = world();
        let members = title_subtitle_members(&world);
        assert_eq!(
            evaluate(&world, &world.not(vec![world.contains("Title", "map")]), &members),
            vec![false, false, true, true]
        );
    }

    #[test]
    fn all_of_is_true_only_when_every_child_is() {
        let world = world();
        let members = title_subtitle_members(&world);
        let all_of =
            world.all_of(vec![world.contains("Title", "map"), world.contains("Subtitle", "map")]);
        assert_eq!(evaluate(&world, &all_of, &members), vec![true, false, false, false]);
    }

    #[test]
    fn any_of_combines_composite_children() {
        let world = world();
        let members = title_subtitle_members(&world);
        // Title without Subtitle, or neither.
        let any_of = world.any_of(vec![
            world.all_of(vec![
                world.contains("Title", "map"),
                world.not(vec![world.contains("Subtitle", "map")]),
            ]),
            world.not(vec![world
                .any_of(vec![world.contains("Title", "map"), world.contains("Subtitle", "map")])]),
        ]);
        assert_eq!(evaluate(&world, &any_of, &members), vec![false, true, false, true]);
    }

    #[test]
    fn singleton_groups_and_double_negation_behave_as_their_child() {
        let world = world();
        let members = title_subtitle_members(&world);
        let leaf = || world.contains("Title", "map");
        let expected = evaluate(&world, &leaf(), &members);
        for predicate in [
            world.all_of(vec![leaf()]),
            world.any_of(vec![leaf()]),
            world.not(vec![world.not(vec![leaf()])]),
        ] {
            assert_eq!(evaluate(&world, &predicate, &members), expected);
        }
    }

    #[test]
    fn nested_mixed_operator_composition() {
        let world = world();
        // Published, and either an atlas Title or no draft Subtitle.
        let predicate = world.all_of(vec![
            world.equals("Status", &world.status, token("Published")),
            world.any_of(vec![
                world.contains("Title", "atlas"),
                world.not(vec![world.contains("Subtitle", "draft")]),
            ]),
        ]);
        let member = |key: &str, title: &str, subtitle: Option<&str>, status: &str| {
            let mut values = vec![("Title", text(title)), ("Status", token(status))];
            if let Some(subtitle) = subtitle {
                values.push(("Subtitle", text(subtitle)));
            }
            world.member(key, &world.book, &values)
        };
        let members = [
            member("atlas-draft", "Atlas", Some("draft"), "Published"),
            member("roads-final", "Roads", Some("final"), "Published"),
            member("roads-bare", "Roads", None, "Published"),
            member("roads-draft", "Roads", Some("draft"), "Published"),
            member("atlas-unpublished", "Atlas", None, "Draft"),
        ];
        assert_eq!(evaluate(&world, &predicate, &members), vec![true, true, true, false, false]);
    }

    #[test]
    fn filter_keeps_order_and_duplicates_with_a_nested_predicate() {
        let world = world();
        let predicate = validate(
            &world.all_of(vec![
                world.contains("Title", "map"),
                world.not(vec![world.contains("Subtitle", "old")]),
            ]),
            &book(&world),
        )
        .unwrap();
        let a = world.member("a", &world.book, &[("Title", text("Map A"))]);
        let b =
            world.member("b", &world.book, &[("Title", text("Map B")), ("Subtitle", text("old"))]);
        let c = world.member("c", &world.book, &[("Title", text("Roadmap"))]);
        let input = vec![c.clone(), a.clone(), b, a.clone()];
        assert_eq!(filter(&input, &predicate).unwrap(), vec![c, a.clone(), a]);
    }

    // ---- Recursive composition: missing values and errors -------------------

    #[test]
    fn negating_an_absent_optional_value_is_true() {
        let world = world();
        let bare = world.member("bare", &world.book, &[("Title", text("t"))]);
        // Even the empty substring needs a value, so the leaf is false.
        let predicate = world.not(vec![world.contains("Subtitle", "")]);
        assert_eq!(evaluate(&world, &predicate, &[bare]), vec![true]);
    }

    #[test]
    fn required_absence_fails_under_every_composite() {
        let world = world();
        let untitled = world.member("untitled", &world.book, &[("Subtitle", text("map"))]);
        let title = || world.contains("Title", "x");
        let subtitle = || world.contains("Subtitle", "map");
        for predicate in [
            world.not(vec![title()]),
            world.all_of(vec![subtitle(), title()]),
            world.any_of(vec![subtitle(), title()]),
        ] {
            let predicate = validate(&predicate, &book(&world)).unwrap();
            assert!(
                matches!(predicate.matches(&untitled), Err(HolonError::EmptyField(name)) if name == "Title")
            );
        }
    }

    #[test]
    fn a_false_all_of_branch_does_not_hide_a_later_error() {
        let world = world();
        let predicate = validate(
            &world.all_of(vec![
                world.equals("PageCount", &world.integer, number(1)),
                world.contains("Title", "x"),
            ]),
            &book(&world),
        )
        .unwrap();
        // The first child is false; the second must still read the required Title.
        let member = world.member("untitled", &world.book, &[("PageCount", number(2))]);
        assert!(matches!(predicate.matches(&member), Err(HolonError::EmptyField(_))));
    }

    #[test]
    fn a_true_any_of_branch_does_not_hide_a_nested_error() {
        let world = world();
        let predicate = validate(
            &world.any_of(vec![
                world.contains("Subtitle", "map"),
                world.not(vec![world.all_of(vec![world.contains("Title", "x")])]),
            ]),
            &book(&world),
        )
        .unwrap();
        let member = world.member("untitled", &world.book, &[("Subtitle", text("map"))]);
        assert!(matches!(predicate.matches(&member), Err(HolonError::EmptyField(_))));
    }

    #[test]
    fn a_representation_error_propagates_through_not() {
        let world = world();
        let predicate =
            validate(&world.not(vec![world.contains("Subtitle", "x")]), &book(&world)).unwrap();
        let member = world.member("a", &world.book, &[("Subtitle", number(3))]);
        assert!(matches!(predicate.matches(&member), Err(HolonError::ValueKindMismatch { .. })));
    }

    // ---- Recursive composition: definition validation -----------------------

    #[test]
    fn not_requires_exactly_one_child() {
        let world = world();
        let error = validate(&world.not(vec![]), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::MissingRequiredRelationship { relationship, .. } if relationship == "Predicate"),
            "unexpected error: {error:?}"
        );

        let two = world.not(vec![world.contains("Title", "a"), world.contains("Title", "b")]);
        let error = validate(&two, &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::MultipleRelatedHolons { relationship, .. } if relationship == "Predicate"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_empty_group_is_invalid_at_any_depth() {
        let world = world();
        for predicate in [
            world.all_of(vec![]),
            world.all_of(vec![world.contains("Title", "x"), world.any_of(vec![])]),
            world.not(vec![world.all_of(vec![])]),
        ] {
            let error = validate(&predicate, &book(&world)).unwrap_err();
            assert!(
                matches!(&error, HolonError::InvalidParameter(detail) if detail.contains("requires one or more Predicates")),
                "unexpected error: {error:?}"
            );
        }
    }

    fn assert_cyclic(world: &PredicateWorld, predicate: &HolonReference) {
        let error = validate(predicate, &book(world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::InvalidParameter(detail) if detail.contains("cyclic")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn a_self_referencing_composite_is_cyclic() {
        let world = world();
        let mut not = world.not(vec![]);
        not.add_related_holons(QueryRelationshipTypeName::Predicate, vec![not.clone()]).unwrap();
        assert_cyclic(&world, &not);

        let mut all_of = world.all_of(vec![world.contains("Title", "x")]);
        all_of
            .add_related_holons(QueryRelationshipTypeName::Predicates, vec![all_of.clone()])
            .unwrap();
        assert_cyclic(&world, &all_of);
    }

    #[test]
    fn a_multi_node_cycle_is_rejected() {
        let world = world();
        // AllOf -> Not -> AnyOf -> AllOf, entered below the root.
        let mut all_of = world.all_of(vec![world.contains("Title", "x")]);
        let any_of = world.any_of(vec![world.contains("Title", "y"), all_of.clone()]);
        let not = world.not(vec![any_of]);
        all_of.add_related_holons(QueryRelationshipTypeName::Predicates, vec![not]).unwrap();
        assert_cyclic(&world, &world.not(vec![all_of]));
    }

    #[test]
    fn a_shared_child_is_not_a_cycle() {
        let world = world();
        let members = title_subtitle_members(&world);
        let shared = world.contains("Title", "map");
        // Reused in separate branches.
        let branches = world.any_of(vec![
            world.all_of(vec![shared.clone(), world.contains("Subtitle", "map")]),
            world.all_of(vec![shared.clone(), world.not(vec![world.contains("Subtitle", "map")])]),
        ]);
        assert_eq!(evaluate(&world, &branches, &members), vec![true, true, false, false]);
        // Repeated within one group.
        let repeated = world.all_of(vec![shared.clone(), shared]);
        assert_eq!(evaluate(&world, &repeated, &members), vec![true, true, false, false]);
    }

    #[test]
    fn an_unsupported_operator_fails_although_another_branch_decides() {
        let world = world();
        // Status is an enum: it affords Equals but no ordering.
        let unordered = world.compare("Status", &world.greater_than, &world.status, token("Draft"));
        let error = validate(
            &world.any_of(vec![world.contains("Title", ""), world.not(vec![unordered])]),
            &book(&world),
        )
        .unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedOperator { operator, .. } if operator == "GreaterThanOperator"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn a_deep_invalid_leaf_fails_validation() {
        let world = world();
        let deep = |leaf: HolonReference| {
            world.all_of(vec![
                world.contains("Title", "x"),
                world.not(vec![world.any_of(vec![world.contains("Subtitle", "y"), leaf])]),
            ])
        };

        let error = validate(&deep(world.contains("Missing", "x")), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { name, .. } if name == "Missing"),
            "unexpected error: {error:?}"
        );
        let error = validate(&deep(world.contains("PageCount", "1")), &book(&world)).unwrap_err();
        assert!(
            matches!(error, HolonError::UnsupportedOperator { .. }),
            "unexpected error: {error:?}"
        );
        let error =
            validate(&deep(world.equals("Title", &world.integer, number(1))), &book(&world))
                .unwrap_err();
        assert!(
            matches!(error, HolonError::IncompatibleLiteralType { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn a_child_that_is_not_a_predicate_is_rejected() {
        let world = world();
        let literal = world.substring("x");
        for predicate in [
            world.any_of(vec![world.contains("Title", "x"), literal.clone()]),
            world.not(vec![literal]),
        ] {
            let error = validate(&predicate, &book(&world)).unwrap_err();
            assert!(
                matches!(&error, HolonError::WrongDescriptorKind { expected, .. } if expected == "QueryPredicate"),
                "unexpected error: {error:?}"
            );
        }
    }

    #[test]
    fn nested_selectors_resolve_on_the_declared_type() {
        let world = world();
        let novel = PredicateWorld::descriptor(&world.novel);
        let narrator = || world.not(vec![world.all_of(vec![world.contains("Narrator", "x")])]);
        validate(&narrator(), &novel).expect("Narrator is declared on Novel");
        validate(&world.not(vec![world.contains("Title", "x")]), &novel)
            .expect("Title is inherited from Book");

        let error = validate(&narrator(), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { name, .. } if name == "Narrator"),
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
    fn an_enum_literal_must_also_be_a_member_of_the_required_type() {
        // A declared subtype can declare a variant its parent lacks. The operator's
        // required type still governs which tokens the comparison accepts.
        use crate::descriptors::test_support::{new_descriptor_holon, new_kind_descriptor};
        let world = world();
        let mut extended =
            new_kind_descriptor(&world.context, "ExtendedStatus", Some(&world.status), false)
                .unwrap();
        extended
            .add_related_holons(
                CoreRelationshipTypeName::Variants,
                vec![new_descriptor_holon(&world.context, "Archived", "Archived", "EnumVariant")
                    .unwrap()
                    .into()],
            )
            .unwrap();
        let extended: HolonReference = extended.into();
        let error = validate(&world.equals("Status", &extended, token("Archived")), &book(&world))
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
    fn equals_on_a_subject_with_an_unsupported_constraint_fails_explicitly() {
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
    fn a_canonical_unsupported_constraint_rejects_equals_on_subtypes() {
        // A well-formed unknown attachment remains fail-closed on inherited literals.
        let world = world();
        world.unknown_constraint(&world.map_string);
        let error =
            validate(&world.equals("Title", &world.text, text("x")), &book(&world)).unwrap_err();
        assert!(
            matches!(&error, HolonError::UnsupportedValueConstraint { constraint_type, .. }
                if constraint_type == "Unknown"),
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
    fn an_extension_of_all_of_is_evaluated_as_one() {
        let world = world();
        let mut extension_type =
            new_holon_type_descriptor(&world.context, "Every.HolonType", "Every").unwrap();
        extension_type
            .add_related_holons(
                CoreRelationshipTypeName::Extends,
                vec![canonical_type(&world.context, "AllOf")],
            )
            .unwrap();
        let mut predicate =
            crate::descriptors::test_support::new_test_holon(&world.context, "every").unwrap();
        predicate.with_descriptor(extension_type.into()).unwrap();
        predicate
            .add_related_holons(
                QueryRelationshipTypeName::Predicates,
                vec![world.contains("Title", "x")],
            )
            .unwrap();
        assert!(matches!(
            validate(&predicate.into(), &book(&world)).unwrap(),
            ResolvedPredicate::AllOf(_)
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

    // ---- Ordered comparisons -----------------------------------------------

    /// Results for a subject before, equal to, and after the comparison value,
    /// in the order of [`PredicateWorld::ordering_operators`].
    const BOUNDARY_RESULTS: [[bool; 3]; 4] =
        [[true, false, false], [false, false, true], [true, true, false], [false, true, true]];

    #[test]
    fn each_ordered_operator_respects_its_boundary_on_integers_and_strings() {
        let world = world();
        let pages = |key: &str, count: i64| {
            world.member(key, &world.book, &[("Title", text("t")), ("PageCount", number(count))])
        };
        let titled =
            |key: &str, title: &str| world.member(key, &world.book, &[("Title", text(title))]);
        let by_pages = [pages("before", 4), pages("equal", 5), pages("after", 6)];
        let by_title = [titled("before", "l"), titled("equal", "m"), titled("after", "n")];

        for (operator, expected) in world.ordering_operators().into_iter().zip(BOUNDARY_RESULTS) {
            let count = world.compare("PageCount", operator, &world.integer, number(5));
            assert_eq!(evaluate(&world, &count, &by_pages), expected, "PageCount");
            let title = world.compare("Title", operator, &world.text, text("m"));
            assert_eq!(evaluate(&world, &title, &by_title), expected, "Title");
        }
    }

    #[test]
    fn ordered_predicates_are_safe_at_the_signed_integer_extremes() {
        let world = world();
        let pages = |key: &str, count: i64| {
            world.member(key, &world.book, &[("Title", text("t")), ("PageCount", number(count))])
        };
        let members = [
            pages("min", i64::MIN),
            pages("minus-one", -1),
            pages("zero", 0),
            pages("max", i64::MAX),
        ];
        let [less_than, greater_than, less_than_or_equal, greater_than_or_equal] =
            world.ordering_operators();
        let at =
            |operator, value| world.compare("PageCount", operator, &world.integer, number(value));

        assert_eq!(
            evaluate(&world, &at(greater_than, i64::MIN), &members),
            vec![false, true, true, true]
        );
        assert_eq!(
            evaluate(&world, &at(less_than, i64::MAX), &members),
            vec![true, true, true, false]
        );
        assert_eq!(evaluate(&world, &at(greater_than_or_equal, i64::MIN), &members), vec![true; 4]);
        assert_eq!(evaluate(&world, &at(less_than_or_equal, i64::MAX), &members), vec![true; 4]);
        assert_eq!(evaluate(&world, &at(less_than, 0), &members), vec![true, true, false, false]);
    }

    #[test]
    fn ordered_string_predicates_are_ordinal_and_independent_of_contains_folding() {
        let world = world();
        let titled =
            |key: &str, title: &str| world.member(key, &world.book, &[("Title", text(title))]);
        let members = [
            titled("empty", ""),
            titled("upper", "STRASSE"),
            titled("folded", "Straße"),
            titled("prefix", "Stra"),
            titled("accented", "élan"),
        ];
        let [less_than, greater_than, less_than_or_equal, greater_than_or_equal] =
            world.ordering_operators();
        let at = |operator, value: &str| world.compare("Title", operator, &world.text, text(value));

        // "" < "STRASSE" < "Stra" < "Straße" < "élan" by code point.
        assert_eq!(
            evaluate(&world, &at(greater_than, "Stra"), &members),
            vec![false, false, true, false, true]
        );
        assert_eq!(
            evaluate(&world, &at(less_than, "Stra"), &members),
            vec![true, true, false, false, false]
        );
        assert_eq!(evaluate(&world, &at(greater_than_or_equal, ""), &members), vec![true; 5]);
        // Contains would fold Straße to STRASSE; ordering compares the originals.
        assert_eq!(
            evaluate(&world, &at(less_than_or_equal, "STRASSE"), &members),
            vec![true, true, false, false, false]
        );
    }

    #[test]
    fn optional_absence_is_false_for_every_ordered_operator() {
        let world = world();
        let untitled = world.member("untitled", &world.book, &[]);
        let titled = world.member("titled", &world.book, &[("Title", text("t"))]);
        for operator in world.ordering_operators() {
            let pages = validate(
                &world.compare("PageCount", operator, &world.integer, number(0)),
                &book(&world),
            )
            .unwrap();
            assert!(!pages.matches(&titled).unwrap(), "an absent PageCount never matches");

            let title =
                validate(&world.compare("Title", operator, &world.text, text("")), &book(&world))
                    .unwrap();
            assert!(matches!(
                title.matches(&untitled),
                Err(HolonError::EmptyField(name)) if name == "Title"
            ));
        }
        // An inclusive comparison is not the negation of its strict opposite.
        let [_, greater_than, less_than_or_equal, _] = world.ordering_operators();
        let at_most = world.compare("PageCount", less_than_or_equal, &world.integer, number(0));
        let not_above =
            world.not(vec![world.compare("PageCount", greater_than, &world.integer, number(0))]);
        assert_eq!(evaluate(&world, &at_most, &[titled.clone()]), vec![false]);
        assert_eq!(evaluate(&world, &not_above, &[titled]), vec![true]);
    }

    #[test]
    fn an_ordered_comparison_rejects_a_stored_value_of_the_wrong_representation() {
        let world = world();
        let predicate = validate(
            &world.compare("PageCount", &world.greater_than, &world.integer, number(1)),
            &book(&world),
        )
        .unwrap();
        let member =
            world.member("a", &world.book, &[("Title", text("t")), ("PageCount", text("2"))]);
        assert!(matches!(predicate.matches(&member), Err(HolonError::ValueKindMismatch { .. })));
    }

    #[test]
    fn ordered_affordances_are_independent_and_inherited() {
        let world = world();
        // Rank affords LessThan only.
        validate(&world.compare("Rank", &world.less_than, &world.rank, number(1)), &book(&world))
            .expect("LessThan is afforded on Rank");
        for operator in
            [&world.greater_than, &world.less_than_or_equal, &world.greater_than_or_equal]
        {
            let error =
                validate(&world.compare("Rank", operator, &world.rank, number(1)), &book(&world))
                    .unwrap_err();
            assert!(
                matches!(&error, HolonError::UnsupportedOperator { value_type, .. } if value_type == "Rank"),
                "unexpected error: {error:?}"
            );
        }
        // Headline declares no affordance of its own and inherits Text's.
        let predicate = validate(
            &world.compare("Headline", &world.greater_than_or_equal, &world.headline, text("M")),
            &book(&world),
        )
        .unwrap();
        let member =
            world.member("a", &world.book, &[("Title", text("t")), ("Headline", text("News"))]);
        assert!(predicate.matches(&member).unwrap());
    }

    #[test]
    fn ordering_is_unavailable_outside_the_integer_and_string_domains() {
        let world = world();
        let domains = [
            ("IsPublished", &world.flag, flag(true)),
            ("Status", &world.status, token("Draft")),
            ("Digest", &world.digest, bytes(&[1])),
            ("RawDigest", &world.raw_digest, bytes(&[1])),
        ];
        let assert_unsupported = |property: &str, value_type, value: &BaseValue| {
            for operator in world.ordering_operators() {
                let comparison = world.compare(property, operator, value_type, value.clone());
                let error = validate(&comparison, &book(&world)).unwrap_err();
                assert!(
                    matches!(error, HolonError::UnsupportedOperator { .. }),
                    "{property}: unexpected error: {error:?}"
                );
            }
        };
        for (property, value_type, value) in &domains {
            assert_unsupported(property, value_type, value);
        }
        // A deliberate ordered affordance does not give these domains ordering.
        for (property, value_type, value) in &domains {
            let HolonReference::Transient(mut holon) = (*value_type).clone() else {
                panic!("{property}'s value type is transient");
            };
            holon
                .add_related_holons(
                    CoreRelationshipTypeName::AffordsOperator,
                    world.ordering_operators().into_iter().cloned().collect(),
                )
                .unwrap();
            assert_unsupported(property, value_type, value);
        }
    }

    #[test]
    fn ordered_literals_are_checked_against_the_required_type() {
        let world = world();
        // A subtype of the subject type is compatible.
        let predicate = validate(
            &world.compare("Title", &world.less_than, &world.headline, text("b")),
            &book(&world),
        )
        .unwrap();
        assert!(predicate
            .matches(&world.member("a", &world.book, &[("Title", text("a"))]))
            .unwrap());

        // Broader or unrelated declared types are not.
        for (property, value_type, value) in [
            ("Title", &world.map_string, text("b")),
            ("Title", &world.integer, number(1)),
            ("PageCount", &world.text, text("1")),
            ("PageCount", &world.rank, number(1)),
        ] {
            let error = validate(
                &world.compare(property, &world.greater_than, value_type, value),
                &book(&world),
            )
            .unwrap_err();
            assert!(
                matches!(error, HolonError::IncompatibleLiteralType { .. }),
                "{property}: unexpected error: {error:?}"
            );
        }

        // A value outside its declared representation, and an unsupported configured constraint.
        let error = validate(
            &world.compare("PageCount", &world.greater_than, &world.integer, text("1")),
            &book(&world),
        )
        .unwrap_err();
        assert!(
            matches!(error, HolonError::ValueKindMismatch { .. }),
            "unexpected error: {error:?}"
        );
        let error = validate(
            &world.compare("Identifier", &world.greater_than, &world.identifier, text("a")),
            &book(&world),
        )
        .unwrap_err();
        assert!(
            matches!(error, HolonError::UnsupportedValueConstraint { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_ordered_operator_takes_exactly_one_operand() {
        let world = world();
        for operator in world.ordering_operators() {
            for operands in [
                vec![],
                vec![
                    world.literal(&world.integer, number(1)),
                    world.literal(&world.integer, number(2)),
                ],
            ] {
                let found = operands.len();
                let error =
                    validate(&world.comparison("PageCount", operator, operands), &book(&world))
                        .unwrap_err();
                assert!(
                    matches!(error, HolonError::OperandCountMismatch { expected: 1, found: f, .. } if f == found),
                    "unexpected error: {error:?}"
                );
            }
        }
    }

    #[test]
    fn ordered_selectors_resolve_on_the_declared_type() {
        let world = world();
        let novel = PredicateWorld::descriptor(&world.novel);
        let predicate =
            validate(&world.compare("Title", &world.greater_than, &world.text, text("m")), &novel)
                .expect("Title is inherited from Book");
        let error = validate(
            &world.compare("Narrator", &world.greater_than, &world.text, text("m")),
            &book(&world),
        )
        .unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { name, .. } if name == "Narrator"),
            "unexpected error: {error:?}"
        );
        // A Novel member in a Book collection is held to Book's contract.
        let member =
            world.member("novel", &world.novel, &[("Title", text("z")), ("Narrator", text("a"))]);
        assert!(predicate.matches(&member).unwrap());
        let by_title = validate(
            &world.compare("Title", &world.greater_than, &world.text, text("m")),
            &book(&world),
        )
        .unwrap();
        assert!(by_title.matches(&member).unwrap());
    }

    #[test]
    fn ordered_leaves_compose_with_equals_and_contains() {
        let world = world();
        let book_of = |key: &str, title: &str, pages: i64, status: &str| {
            world.member(
                key,
                &world.book,
                &[("Title", text(title)), ("PageCount", number(pages)), ("Status", token(status))],
            )
        };
        let members = [
            book_of("long-atlas", "Atlas", 300, "Draft"),
            book_of("long-published", "Roads", 250, "Published"),
            book_of("long-draft", "Roads", 400, "Draft"),
            book_of("short-atlas", "Atlas", 50, "Published"),
        ];
        // PageCount >= 250 and (Title contains "atlas" or not Status = Draft).
        let predicate = world.all_of(vec![
            world.compare("PageCount", &world.greater_than_or_equal, &world.integer, number(250)),
            world.any_of(vec![
                world.contains("Title", "atlas"),
                world.not(vec![world.equals("Status", &world.status, token("Draft"))]),
            ]),
        ]);
        assert_eq!(evaluate(&world, &predicate, &members), vec![true, true, false, false]);

        // A decisive branch hides neither a candidate error ...
        let untitled = world.member("untitled", &world.book, &[("PageCount", number(1))]);
        let decided = validate(
            &world.any_of(vec![
                world.compare("PageCount", &world.less_than, &world.integer, number(5)),
                world.compare("Title", &world.greater_than, &world.text, text("a")),
            ]),
            &book(&world),
        )
        .unwrap();
        assert!(matches!(decided.matches(&untitled), Err(HolonError::EmptyField(_))));
        // ... nor a definition error.
        let error = validate(
            &world.all_of(vec![
                world.compare("PageCount", &world.less_than, &world.integer, number(0)),
                world.compare("Rank", &world.greater_than, &world.rank, number(1)),
            ]),
            &book(&world),
        )
        .unwrap_err();
        assert!(
            matches!(error, HolonError::UnsupportedOperator { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_ordered_filter_keeps_order_and_duplicates_and_leaves_its_definition_unchanged() {
        let world = world();
        let pages = |key: &str, count: i64| {
            world.member(key, &world.book, &[("Title", text("t")), ("PageCount", number(count))])
        };
        let (short, long) = (pages("short", 10), pages("long", 500));
        let comparison =
            world.compare("PageCount", &world.greater_than, &world.integer, number(100));
        let definition = comparison.into_model().unwrap();
        let predicate = validate(&comparison, &book(&world)).unwrap();
        let retained =
            filter(&[long.clone(), short.clone(), long.clone(), short], &predicate).unwrap();
        assert_eq!(retained, vec![long.clone(), long]);
        assert_eq!(comparison.into_model().unwrap(), definition);
    }

    #[test]
    fn contains_literal_bounds_cover_declared_required_and_coincident_types() {
        for location in ["declared", "required", "both", "coincident"] {
            let world = world();
            if ["declared", "both"].contains(&location) {
                world.string_bound(&world.headline, "DeclaredBound", 3);
            }
            if ["required", "both", "coincident"].contains(&location) {
                world.string_bound(&world.map_string, "RequiredBound", 3);
            }
            let declared =
                if location == "coincident" { &world.map_string } else { &world.headline };
            for (value, valid) in [("a\u{301}bc", true), ("abcd", false)] {
                let comparison = world.comparison(
                    "Title",
                    &world.contains,
                    vec![world.literal(declared, text(value))],
                );
                let definition = comparison.into_model().unwrap();
                let result = validate(&comparison, &book(&world));
                if valid {
                    result.unwrap();
                } else {
                    assert!(
                        matches!(result, Err(HolonError::ValueConstraintViolation { ref code, ref value_type, ref detail, .. }) if code == "StringLengthOutOfRange" && value_type == &declared.reference_id_string() && detail.contains("4 graphemes") && detail.contains("[0, 3]")),
                        "{location}: {result:?}"
                    );
                }
                assert_eq!(comparison.into_model().unwrap(), definition);
            }
        }
    }

    #[test]
    fn every_inherited_and_local_literal_bound_is_enforced_with_provenance() {
        let world = world();
        let required = world.string_bound(&world.map_string, "Required", 5);
        let inherited = world.string_bound(&world.text, "Inherited", 3);
        let local = world.string_bound(&world.headline, "Local", 2);
        for (value, constraint, declaring) in [
            ("abcdef", &required, &world.map_string),
            ("abcd", &inherited, &world.text),
            ("abc", &local, &world.headline),
        ] {
            let comparison = world.comparison(
                "Title",
                &world.contains,
                vec![world.literal(&world.headline, text(value))],
            );
            let error = validate(&comparison, &book(&world)).unwrap_err();
            let type_ref = constraint
                .related_holons(CoreRelationshipTypeName::DescribedBy)
                .unwrap()
                .read()
                .unwrap()
                .get_members()[0]
                .clone();
            assert!(
                matches!(error, HolonError::ValueConstraintViolation { constraint: ref actual, constraint_type: ref actual_type, descriptor: ref actual_declaring, .. } if actual == &constraint.reference_id_string() && actual_type == &type_ref.reference_id_string() && actual_declaring == &declaring.reference_id_string()),
                "{error:?}"
            );
        }
        let comparison = world.comparison(
            "Title",
            &world.contains,
            vec![world.literal(&world.headline, text("ab"))],
        );
        validate(&comparison, &book(&world)).unwrap();
    }

    #[test]
    fn contains_does_not_apply_subject_property_bounds_to_its_substring() {
        let world = world();
        world.string_bound(&world.text, "SubjectOnly", 1);
        validate(&world.contains("Title", "substring"), &book(&world)).unwrap();
    }

    #[test]
    fn malformed_constraint_policy_precedes_unsupported_semantics() {
        use core_types::SchemaInvalidityKind;
        for defect in [
            "configuration",
            "applicability",
            "unknown-applicability",
            "abstract",
            "non-constraint",
            "lineage",
            "missing",
            "multiple",
            "unknown-missing",
        ] {
            let world = world();
            let unknown = defect.starts_with("unknown");
            let mut bound = if unknown {
                world.unknown_constraint(&world.map_string)
            } else {
                world.string_bound(&world.map_string, "Bound", 3)
            };
            let mut type_ref = bound
                .related_holons(CoreRelationshipTypeName::DescribedBy)
                .unwrap()
                .read()
                .unwrap()
                .get_members()[0]
                .clone();
            match defect {
                "configuration" => {
                    bound.with_property_value("Maximum", "three").unwrap();
                }
                "applicability" | "unknown-applicability" => {
                    bound.with_property_value("Maximum", "invalid core bound").unwrap();
                    let targets = type_ref
                        .related_holons(CoreRelationshipTypeName::ApplicableToDescriptorTypes)
                        .unwrap()
                        .read()
                        .unwrap()
                        .get_members()
                        .clone();
                    type_ref
                        .remove_related_holons(
                            CoreRelationshipTypeName::ApplicableToDescriptorTypes,
                            targets,
                        )
                        .unwrap();
                }
                "abstract" => {
                    type_ref.with_property_value("IsAbstractType", true).unwrap();
                }
                "non-constraint" | "lineage" => {
                    let parents = type_ref
                        .related_holons(CoreRelationshipTypeName::Extends)
                        .unwrap()
                        .read()
                        .unwrap()
                        .get_members()
                        .clone();
                    type_ref
                        .remove_related_holons(CoreRelationshipTypeName::Extends, parents.clone())
                        .unwrap();
                    if defect == "non-constraint" {
                        let root = parents[0]
                            .related_holons(CoreRelationshipTypeName::Extends)
                            .unwrap()
                            .read()
                            .unwrap()
                            .get_members()[0]
                            .clone();
                        type_ref
                            .add_related_holons(CoreRelationshipTypeName::Extends, vec![root])
                            .unwrap();
                    }
                }
                "missing" | "unknown-missing" | "multiple" => {
                    // Transient definitions can carry malformed describers for validation.
                    let mut malformed = crate::descriptors::test_support::new_test_holon(
                        &world.context,
                        "MalformedBound",
                    )
                    .unwrap();
                    if defect == "multiple" {
                        malformed
                            .add_related_holons(
                                CoreRelationshipTypeName::DescribedBy,
                                vec![type_ref.clone(), world.map_string.clone()],
                            )
                            .unwrap();
                    }
                    let malformed: HolonReference = malformed.into();
                    world
                        .map_string
                        .clone()
                        .remove_related_holons(
                            CoreRelationshipTypeName::Constraints,
                            vec![bound.clone()],
                        )
                        .unwrap();
                    world
                        .text
                        .clone()
                        .add_related_holons(CoreRelationshipTypeName::Constraints, vec![malformed])
                        .unwrap();
                }
                _ => unreachable!(),
            }
            let comparison = world.comparison(
                "Title",
                &world.contains,
                vec![world.literal(&world.text, text("x"))],
            );
            let error = validate(&comparison, &book(&world)).unwrap_err();
            match defect {
                "missing" | "unknown-missing" => {
                    assert!(matches!(error, HolonError::MissingDescribedBy { .. }), "{error:?}")
                }
                "multiple" => assert!(
                    matches!(error, HolonError::MultipleDescribedBy { count: 2, .. }),
                    "{error:?}"
                ),
                _ => {
                    let expected = if defect.ends_with("applicability") {
                        SchemaInvalidityKind::IncompatibleConstraintFamily
                    } else {
                        SchemaInvalidityKind::InvalidConstraintConfiguration
                    };
                    assert!(
                        matches!(error, HolonError::DescriptorSchemaInvalid { kind, ref detail, .. } if kind == expected && detail.contains(&bound.reference_id_string()) && detail.contains(&type_ref.reference_id_string())),
                        "{defect}: {error:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_literal_validation_leaves_definitions_unchanged() {
        let world = world();
        world.string_bound(&world.map_string, "Tight", 1);
        let comparison = world.contains("Title", "too long");
        let comparison_before = comparison.into_model().unwrap();
        let operand = comparison
            .related_holons(QueryRelationshipTypeName::Operands)
            .unwrap()
            .read()
            .unwrap()
            .get_members()[0]
            .clone();
        let operand_before = operand.into_model().unwrap();
        assert!(matches!(
            validate(&comparison, &book(&world)),
            Err(HolonError::ValueConstraintViolation { .. })
        ));
        assert_eq!(comparison.into_model().unwrap(), comparison_before);
        assert_eq!(operand.into_model().unwrap(), operand_before);
    }
    #[test]
    fn later_schema_defects_precede_earlier_literal_violations() {
        for malformed in [false, true] {
            let world = world();
            world.string_bound(&world.map_string, "InheritedBound", 1);
            if malformed {
                let mut bound = world.string_bound(&world.text, "Malformed", 5);
                bound.with_property_value("Maximum", "invalid").unwrap();
            } else {
                world.unknown_constraint(&world.text);
            }
            let comparison = world.comparison(
                "Title",
                &world.contains,
                vec![world.literal(&world.text, text("long"))],
            );
            let error = validate(&comparison, &book(&world)).unwrap_err();
            assert!(
                if malformed {
                    matches!(error, HolonError::DescriptorSchemaInvalid { .. })
                } else {
                    matches!(error, HolonError::UnsupportedValueConstraint { .. })
                },
                "{error:?}"
            );
        }
    }
}
