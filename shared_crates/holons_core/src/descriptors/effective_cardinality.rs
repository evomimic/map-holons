use super::{
    effective_relationship_targets, equals_or_extends, resolve_core_descriptor, Descriptor,
    RelationshipDescriptor,
};
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::BaseValue;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

/// Inclusive bounds admitted by all effective directional cardinality constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveCardinality {
    /// Greatest inclusive minimum across the applicable constraints.
    pub minimum: i64,
    /// No finite upper bound is represented by `None`, never a sentinel.
    pub maximum: Option<i64>,
}

impl RelationshipDescriptor {
    /// Resolves configured bounds without reading the relationship's target population.
    pub fn effective_cardinality(&self) -> Result<EffectiveCardinality, HolonError> {
        let reference = self.holon();
        let context = reference.resolution_context()?;
        let root = resolve_core_descriptor(&context, "CardinalityConstraint.ConstraintType")?;
        let mut bounds = EffectiveCardinality { minimum: 0, maximum: None };
        let mut found = false;
        for occurrence in
            effective_relationship_targets(reference, CoreRelationshipTypeName::Constraints)?
        {
            let constraint = occurrence.member;
            let descriptor = constraint.holon_descriptor()?;
            if !equals_or_extends(descriptor.holon(), &root)? {
                continue;
            }
            let mut applicable = false;
            let targets = descriptor
                .holon()
                .related_holons("ApplicableToDescriptorTypes")?
                .read()
                .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
                .get_members()
                .clone();
            for target in targets {
                if equals_or_extends(reference, &target)? {
                    applicable = true;
                }
            }
            if !applicable {
                return Err(invalid("constraint is not applicable to this descriptor"));
            }
            let minimum =
                bound(&constraint, "Minimum")?.ok_or_else(|| invalid("missing Minimum"))?;
            let maximum = bound(&constraint, "Maximum")?;
            if maximum.is_some_and(|maximum| maximum < minimum) {
                return Err(invalid("Maximum is less than Minimum"));
            }
            bounds.minimum = bounds.minimum.max(minimum);
            if let Some(maximum) = maximum {
                bounds.maximum =
                    Some(bounds.maximum.map_or(maximum, |current| current.min(maximum)));
            }
            found = true;
        }
        if !found {
            return Err(invalid("no applicable CardinalityConstraint"));
        }
        if bounds.maximum.is_some_and(|maximum| maximum < bounds.minimum) {
            return Err(invalid("effective constraints have an empty intersection"));
        }
        Ok(bounds)
    }
}

fn bound(reference: &HolonReference, name: &str) -> Result<Option<i64>, HolonError> {
    match reference.property_value(name)? {
        None => Ok(None),
        Some(BaseValue::IntegerValue(value)) if value.0 >= 0 => Ok(Some(value.0)),
        _ => Err(invalid(&format!("{name} must be a non-negative integer"))),
    }
}
fn invalid(message: &str) -> HolonError {
    HolonError::InvalidParameter(format!("Invalid cardinality: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon, new_test_holon};
    use crate::reference_layer::WritableHolon;
    use std::sync::Arc;

    fn fixture() -> Result<(Arc<TransactionContext>, HolonReference, HolonReference), HolonError> {
        let context = build_context();
        let relationship = new_descriptor_holon(
            &context,
            "RelationshipType.TypeDescriptor",
            "RelationshipType",
            "Relationship",
        )?;
        let relationship: HolonReference = context.mutation().stage_new_holon(relationship)?.into();
        let mut constraint_type = new_descriptor_holon(
            &context,
            "CardinalityConstraint.ConstraintType",
            "CardinalityConstraint",
            "Constraint",
        )?;
        constraint_type
            .add_related_holons("ApplicableToDescriptorTypes", vec![relationship.clone()])?;
        let constraint_type = context.mutation().stage_new_holon(constraint_type)?.into();
        Ok((context, relationship, constraint_type))
    }
    fn constraint(
        context: &Arc<TransactionContext>,
        kind: &HolonReference,
        min: Option<i64>,
        max: Option<i64>,
    ) -> Result<HolonReference, HolonError> {
        let mut c = new_test_holon(context, "arbitrary-constraint-name")?;
        c.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![kind.clone()])?;
        if let Some(n) = min {
            c.with_property_value("Minimum", n)?;
        }
        if let Some(n) = max {
            c.with_property_value("Maximum", n)?;
        }
        Ok(context.mutation().stage_new_holon(c)?.into())
    }
    #[test]
    fn combines_inherited_bounds_and_keeps_direction_independent() -> Result<(), HolonError> {
        let (context, mut parent, kind) = fixture()?;
        parent.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![constraint(&context, &kind, Some(1), None)?],
        )?;
        let mut forward = new_test_holon(&context, "forward")?;
        forward.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.clone()])?;
        forward.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![constraint(&context, &kind, Some(0), Some(1))?],
        )?;
        let mut inverse = new_test_holon(&context, "inverse")?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent])?;
        assert_eq!(
            RelationshipDescriptor::from_holon(forward.into()).effective_cardinality()?,
            EffectiveCardinality { minimum: 1, maximum: Some(1) }
        );
        assert_eq!(
            RelationshipDescriptor::from_holon(inverse.into()).effective_cardinality()?,
            EffectiveCardinality { minimum: 1, maximum: None }
        );
        Ok(())
    }
    #[test]
    fn handles_zero_plural_and_malformed_bounds() -> Result<(), HolonError> {
        for (minimum, maximum, valid) in [
            (Some(0), Some(0), true),
            (Some(2), Some(5), true),
            (Some(0), None, true),
            (None, Some(1), false),
            (Some(-1), None, false),
            (Some(2), Some(1), false),
        ] {
            let (context, mut relationship, kind) = fixture()?;
            relationship.add_related_holons(
                CoreRelationshipTypeName::Constraints,
                vec![constraint(&context, &kind, minimum, maximum)?],
            )?;
            let result = RelationshipDescriptor::from_holon(relationship).effective_cardinality();
            assert_eq!(result.is_ok(), valid, "{minimum:?}..{maximum:?}: {result:?}");
        }
        Ok(())
    }
    #[test]
    fn recognizes_constraint_subtypes_by_identity_and_rejects_inapplicable_constraints(
    ) -> Result<(), HolonError> {
        let (context, mut relationship, kind) = fixture()?;
        let mut subtype =
            new_descriptor_holon(&context, "Custom.ConstraintType", "Custom", "Constraint")?;
        subtype.add_related_holons(CoreRelationshipTypeName::Extends, vec![kind])?;
        subtype.add_related_holons("ApplicableToDescriptorTypes", vec![relationship.clone()])?;
        let subtype: HolonReference = context.mutation().stage_new_holon(subtype)?.into();
        relationship.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![constraint(&context, &subtype, Some(0), Some(1))?],
        )?;
        assert_eq!(
            RelationshipDescriptor::from_holon(relationship.clone())
                .effective_cardinality()?
                .maximum,
            Some(1)
        );
        let mut subtype = subtype;
        subtype.remove_related_holons("ApplicableToDescriptorTypes", vec![relationship.clone()])?;
        assert!(RelationshipDescriptor::from_holon(relationship).effective_cardinality().is_err());
        Ok(())
    }
    #[test]
    fn rejects_missing_constraints_and_empty_intersection() -> Result<(), HolonError> {
        let (context, mut relationship, kind) = fixture()?;
        assert!(RelationshipDescriptor::from_holon(relationship.clone())
            .effective_cardinality()
            .is_err());
        relationship.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![
                constraint(&context, &kind, Some(2), None)?,
                constraint(&context, &kind, Some(0), Some(1))?,
            ],
        )?;
        assert!(RelationshipDescriptor::from_holon(relationship).effective_cardinality().is_err());
        Ok(())
    }
}
