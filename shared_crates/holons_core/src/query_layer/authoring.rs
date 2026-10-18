//! Narrow transient constructors for predicate-bearing query definitions.
//!
//! These build the canonical holonic representation of a `LiteralOperand`, a
//! `ComparisonPredicate`, an `AnyOf`, and a `Filter`, each described by its
//! canonical Query Schema descriptor resolved by key. Construction uses
//! descriptor lookups only to produce that representation; it is not a
//! substitute for execution-time validation, which revalidates the whole
//! graph against the input collection's declared `ElementType`.

use std::sync::Arc;

use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::{
    holon_type_descriptor_key, CoreRelationshipTypeName, QueryHolonTypeName, QueryPropertyTypeName,
    QueryRelationshipTypeName, ToOperatorName, ToPropertyName,
};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::{
    require_holon_type, resolve_core_descriptor, Descriptor, HolonDescriptor, ValueDescriptor,
};
use crate::reference_layer::{HolonReference, TransientReference, WritableHolon};

/// Builds a transient `LiteralOperand` declaring `value` as `value_type`.
///
/// Declare the type the operator requires at the operand's position (or a
/// subtype of it), not necessarily the selected property's ValueType.
pub fn new_literal_operand(
    context: &Arc<TransactionContext>,
    value_type: &ValueDescriptor,
    value: BaseValue,
) -> Result<TransientReference, HolonError> {
    let mut operand =
        new_described(context, "literal-operand", QueryHolonTypeName::LiteralOperand)?;
    operand.with_property_value(QueryPropertyTypeName::OperandValue, value)?;
    operand.add_related_holons(
        CoreRelationshipTypeName::ValueType,
        vec![value_type.holon().clone()],
    )?;
    Ok(operand)
}

/// Builds a transient `ComparisonPredicate` selecting `property` on
/// `element_type` and applying `operator` with `ordered_authored_operands`.
///
/// The operator is resolved by name among the operators the selected
/// property's ValueType affords, so an undeclared property or unafforded
/// operator fails here with the descriptor's own error. Each operand must be a
/// `LiteralOperand` (or an extension of it); their order is preserved.
pub fn new_comparison_predicate<P: ToPropertyName, O: ToOperatorName>(
    context: &Arc<TransactionContext>,
    element_type: &HolonDescriptor,
    property: P,
    operator: O,
    ordered_authored_operands: Vec<HolonReference>,
) -> Result<TransientReference, HolonError> {
    let property_name = property.to_property_name();
    let operator = element_type
        .get_property_by_name(property_name.clone())?
        .value_type()?
        .get_operator_by_name(operator)?;
    for operand in &ordered_authored_operands {
        require_holon_type(operand, &QueryHolonTypeName::LiteralOperand.as_holon_name())?;
    }

    let mut predicate = new_described(
        context,
        &format!("comparison-predicate-{property_name}"),
        QueryHolonTypeName::ComparisonPredicate,
    )?;
    predicate.with_property_value(QueryPropertyTypeName::PropertyName, property_name.0)?;
    predicate
        .add_related_holons(QueryRelationshipTypeName::Operator, vec![operator.holon().clone()])?;
    if !ordered_authored_operands.is_empty() {
        predicate
            .add_related_holons(QueryRelationshipTypeName::Operands, ordered_authored_operands)?;
    }
    Ok(predicate)
}

/// Builds a transient `AnyOf` over `predicates`, in order.
///
/// An empty group is refused: the absence of a filter is expressed by omitting
/// the filter, never by an empty disjunction.
pub fn new_any_of(
    context: &Arc<TransactionContext>,
    predicates: Vec<HolonReference>,
) -> Result<TransientReference, HolonError> {
    if predicates.is_empty() {
        return Err(HolonError::InvalidParameter(
            "AnyOf requires one or more Predicates".to_string(),
        ));
    }
    let mut any_of = new_described(context, "any-of", QueryHolonTypeName::AnyOf)?;
    any_of.add_related_holons(QueryRelationshipTypeName::Predicates, predicates)?;
    Ok(any_of)
}

/// Builds a transient `Filter` expression applying `predicate`.
pub fn new_filter(
    context: &Arc<TransactionContext>,
    predicate: HolonReference,
) -> Result<TransientReference, HolonError> {
    let mut filter = new_described(context, "filter", QueryHolonTypeName::Filter)?;
    filter.add_related_holons(QueryRelationshipTypeName::Predicate, vec![predicate])?;
    Ok(filter)
}

/// A transient holon described by the canonical Query Schema type `type_name`.
fn new_described(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: QueryHolonTypeName,
) -> Result<TransientReference, HolonError> {
    let mut holon = context.mutation().new_holon(Some(MapString(key.to_string())))?;
    holon.with_descriptor(resolve_core_descriptor(
        context,
        &holon_type_descriptor_key(&type_name.as_holon_name()).0,
    )?)?;
    Ok(holon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::accessor_helpers::{related_members, require_single_related};
    use crate::descriptors::test_support::build_context;
    use crate::query_layer::predicate::{filter, validate};
    use crate::query_layer::predicate_test_support::{text, PredicateWorld};
    use crate::reference_layer::ReadableHolon;
    use core_types::PropertyName;
    use type_names::{CoreOperatorTypeName, OperatorName};

    fn world() -> PredicateWorld {
        PredicateWorld::new(build_context())
    }

    fn book(world: &PredicateWorld) -> HolonDescriptor {
        PredicateWorld::descriptor(&world.book)
    }

    #[test]
    fn constructed_definitions_validate_and_execute() {
        let world = world();
        let substring = |value: &str| -> HolonReference {
            new_literal_operand(
                &world.context,
                &ValueDescriptor::from_holon(world.map_string.clone()),
                text(value),
            )
            .unwrap()
            .into()
        };
        let title = new_comparison_predicate(
            &world.context,
            &book(&world),
            "Title",
            CoreOperatorTypeName::ContainsOperator,
            vec![substring("atlas")],
        )
        .unwrap();
        let subtitle = new_comparison_predicate(
            &world.context,
            &book(&world),
            "Subtitle",
            CoreOperatorTypeName::ContainsOperator,
            vec![substring("atlas")],
        )
        .unwrap();
        let any_of = new_any_of(&world.context, vec![title.into(), subtitle.into()]).unwrap();
        let filter_expression = new_filter(&world.context, any_of.into()).unwrap();

        let predicate = validate(
            &require_single_related(
                &filter_expression.into(),
                QueryRelationshipTypeName::Predicate,
            )
            .unwrap(),
            &book(&world),
        )
        .unwrap();
        let by_title = world.member("by-title", &world.book, &[("Title", text("Atlas"))]);
        let by_subtitle = world.member(
            "by-subtitle",
            &world.book,
            &[("Title", text("Maps")), ("Subtitle", text("A world atlas"))],
        );
        let neither = world.member("neither", &world.book, &[("Title", text("Maps"))]);
        assert_eq!(
            filter(&[by_title.clone(), neither, by_subtitle.clone()], &predicate).unwrap(),
            vec![by_title, by_subtitle]
        );
    }

    #[test]
    fn operator_and_property_input_forms_build_identical_predicates() {
        let world = world();
        let build = |property: PropertyName, operator: OperatorName| -> HolonReference {
            new_comparison_predicate(
                &world.context,
                &book(&world),
                property,
                operator,
                vec![world.substring("x")],
            )
            .unwrap()
            .into()
        };
        let forms = [
            new_comparison_predicate(
                &world.context,
                &book(&world),
                "Title",
                CoreOperatorTypeName::ContainsOperator,
                vec![world.substring("x")],
            )
            .unwrap()
            .into(),
            new_comparison_predicate(
                &world.context,
                &book(&world),
                MapString("Title".to_string()),
                "ContainsOperator",
                vec![world.substring("x")],
            )
            .unwrap()
            .into(),
            build(
                PropertyName(MapString("Title".to_string())),
                OperatorName(MapString("ContainsOperator".to_string())),
            ),
        ];
        for predicate in &forms {
            assert_eq!(
                predicate.property_value(QueryPropertyTypeName::PropertyName).unwrap(),
                Some(text("Title"))
            );
            assert_eq!(
                require_single_related(predicate, QueryRelationshipTypeName::Operator).unwrap(),
                world.contains
            );
            assert_eq!(
                related_members(predicate, QueryRelationshipTypeName::Operands).unwrap().len(),
                1
            );
        }
    }

    #[test]
    fn an_unafforded_operator_or_undeclared_property_fails_construction() {
        let world = world();
        let error = new_comparison_predicate(
            &world.context,
            &book(&world),
            "PageCount",
            CoreOperatorTypeName::ContainsOperator,
            vec![world.substring("1")],
        )
        .unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { kind, .. } if kind == "operator"),
            "unexpected error: {error:?}"
        );

        let error = new_comparison_predicate(
            &world.context,
            &book(&world),
            "Narrator",
            CoreOperatorTypeName::ContainsOperator,
            vec![world.substring("x")],
        )
        .unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { kind, .. } if kind == "property"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn operands_must_be_literal_operands() {
        let world = world();
        let not_a_literal = world.contains("Title", "x");
        let error = new_comparison_predicate(
            &world.context,
            &book(&world),
            "Title",
            CoreOperatorTypeName::ContainsOperator,
            vec![not_a_literal],
        )
        .unwrap_err();
        assert!(
            matches!(&error, HolonError::WrongDescriptorKind { expected, .. } if expected == "LiteralOperand"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn an_empty_any_of_is_refused() {
        let world = world();
        assert!(matches!(
            new_any_of(&world.context, Vec::new()),
            Err(HolonError::InvalidParameter(_))
        ));
    }
}
