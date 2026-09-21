use crate::descriptors::{
    accessor_helpers, DeclaredRelationshipDescriptor, Descriptor, HolonDescriptor,
    InverseRelationshipDescriptor, TypeHeader,
};
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, RelationshipName};
use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

/// Runtime wrapper for relationship descriptors.
///
/// Relationship-specific structural and inverse-link behavior will accumulate
/// here in later phases while the wrapper itself stays just a typed view.
pub struct RelationshipDescriptor {
    holon: HolonReference,
}

/// Physical target semantics selected by a completed relationship descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetBinding {
    Version,
    Lineage,
}

/// Inclusive bounds admitted by directly attached directional cardinality constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveCardinality {
    /// Greatest inclusive minimum across the direct cardinality constraints.
    pub minimum: i64,
    /// No finite upper bound is represented by `None`, never a sentinel.
    pub maximum: Option<i64>,
}

impl RelationshipDescriptor {
    /// Combines direct cardinality bounds without resolving inheritance or applicability.
    /// Reads only configured constraints, never the relationship's target population.
    pub fn effective_cardinality(&self) -> Result<EffectiveCardinality, HolonError> {
        let reference = self.holon();
        // Snapshot members before resolving their descriptors to avoid re-entrant locks.
        let constraints = reference
            .related_holons(CoreRelationshipTypeName::Constraints)?
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
            .get_members()
            .clone();
        let mut bounds = EffectiveCardinality { minimum: 0, maximum: None };
        let mut found = false;
        for constraint in constraints {
            if constraint.holon_descriptor()?.header().type_name()?
                != CoreHolonTypeName::CardinalityConstraint.as_holon_name()
            {
                continue;
            }
            let minimum = cardinality_bound(&constraint, "Minimum")?
                .ok_or_else(|| invalid_cardinality("missing Minimum"))?;
            let maximum = cardinality_bound(&constraint, "Maximum")?;
            if maximum.is_some_and(|maximum| maximum < minimum) {
                return Err(invalid_cardinality("Maximum is less than Minimum"));
            }
            bounds.minimum = bounds.minimum.max(minimum);
            if let Some(maximum) = maximum {
                bounds.maximum =
                    Some(bounds.maximum.map_or(maximum, |current| current.min(maximum)));
            }
            found = true;
        }
        if !found {
            return Err(invalid_cardinality("no direct CardinalityConstraint"));
        }
        if bounds.maximum.is_some_and(|maximum| maximum < bounds.minimum) {
            return Err(invalid_cardinality(
                "direct cardinality constraints have an empty intersection",
            ));
        }
        Ok(bounds)
    }

    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns whether the relationship participates in defining identity or structure.
    pub fn is_definitional(&self) -> Result<bool, HolonError> {
        accessor_helpers::relationship_is_definitional(&self.holon)
    }

    /// Maximum age of cached mutable membership in milliseconds. Missing or zero
    /// requires fresh reads. This read policy is inherited along descriptor Extends.
    pub fn membership_cache_max_age_millis(&self) -> Result<u64, HolonError> {
        match accessor_helpers::effective_property_value(
            &self.holon,
            "MembershipCacheMaxAgeMillis",
        )? {
            None => Ok(0),
            Some(base_types::BaseValue::IntegerValue(value)) if value.0 >= 0 => Ok(value.0 as u64),
            _ => Err(HolonError::InvalidParameter(
                "MembershipCacheMaxAgeMillis must be a nonnegative integer".into(),
            )),
        }
    }

    /// Returns whether related members have schema-significant order.
    pub fn is_ordered(&self) -> Result<bool, HolonError> {
        accessor_helpers::relationship_is_ordered(&self.holon)
    }

    /// Returns whether repeated target references are allowed.
    pub fn allows_duplicates(&self) -> Result<bool, HolonError> {
        accessor_helpers::relationship_allows_duplicates(&self.holon)
    }

    /// Returns the completed physical-target binding declared for this direction.
    pub fn target_binding(&self) -> Result<TargetBinding, HolonError> {
        match accessor_helpers::relationship_target_binding(&self.holon)?.0.as_str() {
            "Version" => Ok(TargetBinding::Version),
            "Lineage" => Ok(TargetBinding::Lineage),
            other => Err(HolonError::InvalidParameter(format!(
                "Relationship descriptor has invalid TargetBinding enum value {other:?}"
            ))),
        }
    }

    /// Returns the optional deletion semantic declared by this relationship, when populated.
    pub fn deletion_semantic(&self) -> Result<Option<MapString>, HolonError> {
        accessor_helpers::relationship_deletion_semantic(&self.holon)
    }

    /// Returns this descriptor's base relationship name.
    pub fn base_relationship_name(&self) -> Result<RelationshipName, HolonError> {
        accessor_helpers::relationship_base_relationship_name(&self.holon)
    }

    /// Returns the source holon descriptor reached through the required `SourceType` relationship.
    pub fn source_type(&self) -> Result<HolonDescriptor, HolonError> {
        accessor_helpers::relationship_source_type(&self.holon)
    }

    /// Returns the target holon descriptor reached through the required `TargetType` relationship.
    pub fn target_type(&self) -> Result<HolonDescriptor, HolonError> {
        accessor_helpers::relationship_target_type(&self.holon)
    }

    /// Returns the full `(Source)-[Base]->(Target)` relationship name.
    pub fn full_relationship_name(&self) -> Result<MapString, HolonError> {
        accessor_helpers::relationship_full_relationship_name(&self.holon)
    }

    /// Narrows this descriptor to a declared relationship descriptor.
    pub fn try_into_declared_relationship_descriptor(
        self,
    ) -> Result<DeclaredRelationshipDescriptor, HolonError> {
        DeclaredRelationshipDescriptor::try_from_holon(self.holon)
    }

    /// Narrows this descriptor to an inverse relationship descriptor.
    pub fn try_into_inverse_relationship_descriptor(
        self,
    ) -> Result<InverseRelationshipDescriptor, HolonError> {
        InverseRelationshipDescriptor::try_from_holon(self.holon)
    }
}

impl From<HolonReference> for RelationshipDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for RelationshipDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<RelationshipDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_descriptor_holon, new_test_holon,
    };
    use crate::reference_layer::WritableHolon;
    use base_types::{MapEnumValue, MapString};
    use core_types::HolonError;
    use type_names::{CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "relationship-descriptor",
            "RelationshipType",
            "Relationship",
        )?);

        let descriptor = RelationshipDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("RelationshipType".to_string()));

        Ok(())
    }

    #[test]
    fn structural_accessors_return_declared_values() -> Result<(), HolonError> {
        let context = build_context();
        let source_type = new_descriptor_holon(&context, "book-type", "Book", "Holon")?;
        let target_type = new_descriptor_holon(&context, "author-type", "Author", "Holon")?;
        let mut holon =
            new_descriptor_holon(&context, "written-by-relationship", "WrittenBy", "Relationship")?;
        holon
            .with_property_value(CorePropertyTypeName::IsDefinitional, true)?
            .with_property_value(CorePropertyTypeName::IsOrdered, false)?
            .with_property_value(CorePropertyTypeName::AllowsDuplicates, false)?
            .with_property_value(CorePropertyTypeName::DeletionSemantic, "Block")?;
        holon.add_related_holons(CoreRelationshipTypeName::SourceType, vec![source_type.into()])?;
        holon.add_related_holons(CoreRelationshipTypeName::TargetType, vec![target_type.into()])?;

        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(descriptor.is_definitional()?);
        assert!(!descriptor.is_ordered()?);
        assert!(!descriptor.allows_duplicates()?);
        assert_eq!(descriptor.deletion_semantic()?, Some(MapString("Block".to_string())));
        assert_eq!(descriptor.base_relationship_name()?.to_string(), "WrittenBy");
        assert_eq!(descriptor.source_type()?.header().type_name()?, MapString("Book".to_string()));
        assert_eq!(
            descriptor.target_type()?.header().type_name()?,
            MapString("Author".to_string())
        );
        assert_eq!(
            descriptor.full_relationship_name()?,
            MapString("(Book)-[WrittenBy]->(Author)".to_string())
        );

        Ok(())
    }

    #[test]
    fn boolean_accessors_error_when_required_fields_are_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(
            &context,
            "relationship-missing-booleans",
            "MissingBooleans",
            "Relationship",
        )?;
        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.is_definitional(),
            Err(HolonError::EmptyField(field)) if field == "IsDefinitional"
        ));
        assert!(matches!(
            descriptor.is_ordered(),
            Err(HolonError::EmptyField(field)) if field == "IsOrdered"
        ));
        assert!(matches!(
            descriptor.allows_duplicates(),
            Err(HolonError::EmptyField(field)) if field == "AllowsDuplicates"
        ));

        Ok(())
    }

    #[test]
    fn boolean_accessors_error_when_required_fields_have_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "relationship-wrong-booleans",
            "WrongBooleans",
            "Relationship",
        )?;
        holon
            .with_property_value(CorePropertyTypeName::IsDefinitional, "not-a-boolean")?
            .with_property_value(CorePropertyTypeName::IsOrdered, "not-a-boolean")?
            .with_property_value(CorePropertyTypeName::AllowsDuplicates, "not-a-boolean")?;
        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.is_definitional(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Boolean"
        ));
        assert!(matches!(
            descriptor.is_ordered(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Boolean"
        ));
        assert!(matches!(
            descriptor.allows_duplicates(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Boolean"
        ));

        Ok(())
    }

    #[test]
    fn deletion_semantic_returns_none_when_absent() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(
            &context,
            "relationship-without-deletion-semantic",
            "RelatedTo",
            "Relationship",
        )?;
        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert_eq!(descriptor.deletion_semantic()?, None);

        Ok(())
    }

    #[test]
    fn deletion_semantic_accepts_enum_values() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "relationship-with-enum-deletion-semantic",
            "RelatedTo",
            "Relationship",
        )?;
        holon.with_property_value(
            CorePropertyTypeName::DeletionSemantic,
            MapEnumValue(MapString("Cascade".to_string())),
        )?;
        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert_eq!(descriptor.deletion_semantic()?, Some(MapString("Cascade".to_string())));

        Ok(())
    }

    #[test]
    fn deletion_semantic_errors_when_populated_with_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "relationship-with-wrong-deletion-semantic",
            "RelatedTo",
            "Relationship",
        )?;
        holon.with_property_value(CorePropertyTypeName::DeletionSemantic, true)?;
        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.deletion_semantic(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "String"
        ));

        Ok(())
    }

    #[test]
    fn base_relationship_name_errors_when_type_name_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_test_holon(&context, "relationship-without-type-name")?;
        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.base_relationship_name(),
            Err(HolonError::EmptyField(field)) if field == "TypeName"
        ));

        Ok(())
    }

    #[test]
    fn base_relationship_name_errors_when_type_name_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_test_holon(&context, "relationship-wrong-type-name")?;
        holon.with_property_value(CorePropertyTypeName::TypeName, true)?;
        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.base_relationship_name(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "String"
        ));

        Ok(())
    }

    #[test]
    fn required_singular_navigation_errors_when_targets_are_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(
            &context,
            "missing-source-target-types",
            "MissingSourceTargetTypes",
            "Relationship",
        )?;
        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.source_type(),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "SourceType"
        ));
        assert!(matches!(
            descriptor.target_type(),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "TargetType"
        ));

        Ok(())
    }

    #[test]
    fn required_singular_navigation_errors_when_multiple_targets_exist() -> Result<(), HolonError> {
        let context = build_context();
        let source_a = new_descriptor_holon(&context, "source-a", "SourceA", "Holon")?;
        let source_b = new_descriptor_holon(&context, "source-b", "SourceB", "Holon")?;
        let target_a = new_descriptor_holon(&context, "target-a", "TargetA", "Holon")?;
        let target_b = new_descriptor_holon(&context, "target-b", "TargetB", "Holon")?;
        let mut holon = new_descriptor_holon(
            &context,
            "multiple-source-types",
            "MultipleSourceTypes",
            "Relationship",
        )?;
        holon.add_related_holons(
            CoreRelationshipTypeName::SourceType,
            vec![source_a.into(), source_b.into()],
        )?;
        holon.add_related_holons(
            CoreRelationshipTypeName::TargetType,
            vec![target_a.into(), target_b.into()],
        )?;

        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.source_type(),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "SourceType" && count == 2
        ));
        assert!(matches!(
            descriptor.target_type(),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "TargetType" && count == 2
        ));

        Ok(())
    }

    #[test]
    fn narrowing_convenience_methods_validate_subtype_kind() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-type-for-narrowing",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-type-for-narrowing",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut declared = new_descriptor_holon(
            &context,
            "declared-narrowing",
            "DeclaredNarrowing",
            "Relationship",
        )?;
        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        let mut inverse = new_descriptor_holon(
            &context,
            "inverse-narrowing",
            "InverseNarrowing",
            "Relationship",
        )?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;

        assert_eq!(
            RelationshipDescriptor::from_holon(declared.into())
                .try_into_declared_relationship_descriptor()?
                .header()
                .type_name()?,
            MapString("DeclaredNarrowing".to_string())
        );
        assert_eq!(
            RelationshipDescriptor::from_holon(inverse.into())
                .try_into_inverse_relationship_descriptor()?
                .header()
                .type_name()?,
            MapString("InverseNarrowing".to_string())
        );

        Ok(())
    }
}

fn cardinality_bound(reference: &HolonReference, name: &str) -> Result<Option<i64>, HolonError> {
    match reference.property_value(name)? {
        None => Ok(None),
        Some(BaseValue::IntegerValue(value)) if value.0 >= 0 => Ok(Some(value.0)),
        _ => Err(invalid_cardinality(&format!("{name} must be a non-negative integer"))),
    }
}
fn invalid_cardinality(message: &str) -> HolonError {
    HolonError::InvalidParameter(format!("Invalid cardinality: {message}"))
}

#[cfg(test)]
mod cardinality_tests {
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
        let constraint_type = new_descriptor_holon(
            &context,
            "test-cardinality-descriptor-without-canonical-key",
            "CardinalityConstraint",
            "Constraint",
        )?;
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
    fn combines_direct_bounds_and_keeps_direction_independent() -> Result<(), HolonError> {
        let (context, mut forward, kind) = fixture()?;
        forward.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![
                constraint(&context, &kind, Some(1), None)?,
                constraint(&context, &kind, Some(0), Some(1))?,
            ],
        )?;
        let mut inverse = new_test_holon(&context, "inverse")?;
        inverse.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![constraint(&context, &kind, Some(0), None)?],
        )?;
        assert_eq!(
            RelationshipDescriptor::from_holon(forward).effective_cardinality()?,
            EffectiveCardinality { minimum: 1, maximum: Some(1) }
        );
        assert_eq!(
            RelationshipDescriptor::from_holon(inverse.into()).effective_cardinality()?,
            EffectiveCardinality { minimum: 0, maximum: None }
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
    fn ignores_other_constraint_types_without_resolving_ancestry() -> Result<(), HolonError> {
        let (context, mut relationship, kind) = fixture()?;
        let other = new_descriptor_holon(&context, "Other.ConstraintType", "Other", "Constraint")?;
        let other: HolonReference = context.mutation().stage_new_holon(other)?.into();
        relationship.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![
                constraint(&context, &other, None, None)?,
                constraint(&context, &kind, Some(0), Some(1))?,
            ],
        )?;
        assert_eq!(
            RelationshipDescriptor::from_holon(relationship).effective_cardinality()?,
            EffectiveCardinality { minimum: 0, maximum: Some(1) }
        );
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
