use crate::descriptors::{
    accessor_helpers, Descriptor, HolonDescriptor, InverseRelationshipDescriptor, TypeHeader,
};
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::{HolonError, RelationshipName};
use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

/// Runtime wrapper for declared relationship descriptors.
///
/// Construction validates that the descriptor's effective `Extends` chain reaches
/// [`CoreHolonTypeName::DeclaredRelationshipType`].
pub struct DeclaredRelationshipDescriptor {
    holon: HolonReference,
}

impl DeclaredRelationshipDescriptor {
    /// Wraps a relationship descriptor only if it extends
    /// [`CoreHolonTypeName::DeclaredRelationshipType`].
    pub fn try_from_holon(holon: HolonReference) -> Result<Self, HolonError> {
        accessor_helpers::validate_extends_chain_reaches(
            &holon,
            &CoreHolonTypeName::DeclaredRelationshipType.as_holon_name(),
        )?;
        Ok(Self { holon })
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns whether the relationship participates in defining identity or structure.
    pub fn is_definitional(&self) -> Result<bool, HolonError> {
        accessor_helpers::relationship_is_definitional(&self.holon)
    }

    /// Returns whether related members have schema-significant order.
    pub fn is_ordered(&self) -> Result<bool, HolonError> {
        accessor_helpers::relationship_is_ordered(&self.holon)
    }

    /// Returns whether repeated target references are allowed.
    pub fn allows_duplicates(&self) -> Result<bool, HolonError> {
        accessor_helpers::relationship_allows_duplicates(&self.holon)
    }

    /// Returns the minimum number of targets permitted by this relationship.
    pub fn min_cardinality(&self) -> Result<i64, HolonError> {
        accessor_helpers::relationship_min_cardinality(&self.holon)
    }

    /// Returns the maximum number of targets permitted by this relationship.
    pub fn max_cardinality(&self) -> Result<i64, HolonError> {
        accessor_helpers::relationship_max_cardinality(&self.holon)
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

    /// Returns the inverse descriptor when a `HasInverse` edge is populated.
    ///
    /// `HasInverse` is authored as a required singular relationship on declared
    /// relationship descriptors. Absence is represented as `Ok(None)` so callers
    /// can choose the appropriate contract error; multiple targets fail loudly.
    pub fn has_inverse(&self) -> Result<Option<InverseRelationshipDescriptor>, HolonError> {
        accessor_helpers::optional_single_related(
            &self.holon,
            CoreRelationshipTypeName::HasInverse,
        )?
        .map(InverseRelationshipDescriptor::try_from_holon)
        .transpose()
    }
}

impl Descriptor for DeclaredRelationshipDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<DeclaredRelationshipDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_descriptor_holon,
    };
    use crate::reference_layer::WritableHolon;
    use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

    #[test]
    fn try_from_holon_accepts_declared_relationship_chain() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let mut concrete =
            new_descriptor_holon(&context, "authored-by", "AuthoredBy", "Relationship")?;
        concrete
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;

        let descriptor = DeclaredRelationshipDescriptor::try_from_holon(concrete.into())?;

        assert_eq!(descriptor.header().type_name()?, MapString("AuthoredBy".to_string()));

        Ok(())
    }

    #[test]
    fn try_from_holon_accepts_multi_step_declared_relationship_chain() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-relationship-type-multi-step",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let mut concrete_parent =
            new_descriptor_holon(&context, "concrete-parent", "ConcreteParent", "Relationship")?;
        concrete_parent
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        let mut concrete =
            new_descriptor_holon(&context, "authored-by-multi-step", "AuthoredBy", "Relationship")?;
        concrete
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![concrete_parent.into()])?;

        let descriptor = DeclaredRelationshipDescriptor::try_from_holon(concrete.into())?;

        assert_eq!(descriptor.header().type_name()?, MapString("AuthoredBy".to_string()));

        Ok(())
    }

    #[test]
    fn try_from_holon_rejects_wrong_relationship_kind() -> Result<(), HolonError> {
        let context = build_context();
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut concrete =
            new_descriptor_holon(&context, "authored-books", "AuthoredBooks", "Relationship")?;
        concrete
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;

        assert!(matches!(
            DeclaredRelationshipDescriptor::try_from_holon(concrete.into()),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType)
                    && found == "AuthoredBooks"
        ));

        Ok(())
    }

    #[test]
    fn has_inverse_returns_present_or_absent_inverse() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-type-for-has-inverse",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-type-for-has-inverse",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut inverse =
            new_descriptor_holon(&context, "books-by-author", "BooksByAuthor", "Relationship")?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        let mut declared =
            new_descriptor_holon(&context, "author-of-book", "AuthorOfBook", "Relationship")?;
        declared.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![declared_type.clone().into()],
        )?;
        declared.add_related_holons(CoreRelationshipTypeName::HasInverse, vec![inverse.into()])?;

        let descriptor = DeclaredRelationshipDescriptor::try_from_holon(declared.into())?;
        let inverse_descriptor = descriptor.has_inverse()?.expect("inverse should be present");
        assert_eq!(
            inverse_descriptor.header().type_name()?,
            MapString("BooksByAuthor".to_string())
        );

        let mut without_inverse =
            new_descriptor_holon(&context, "without-inverse", "WithoutInverse", "Relationship")?;
        without_inverse
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        let descriptor_without_inverse =
            DeclaredRelationshipDescriptor::try_from_holon(without_inverse.into())?;

        assert!(descriptor_without_inverse.has_inverse()?.is_none());

        Ok(())
    }

    #[test]
    fn has_inverse_errors_when_multiple_targets_exist() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-type-for-multiple-has-inverse",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-type-for-multiple-has-inverse",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut inverse_a =
            new_descriptor_holon(&context, "inverse-a", "InverseA", "Relationship")?;
        inverse_a.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![inverse_type.clone().into()],
        )?;
        let mut inverse_b =
            new_descriptor_holon(&context, "inverse-b", "InverseB", "Relationship")?;
        inverse_b
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        let mut declared =
            new_descriptor_holon(&context, "has-two-inverses", "HasTwoInverses", "Relationship")?;
        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        declared.add_related_holons(
            CoreRelationshipTypeName::HasInverse,
            vec![inverse_a.into(), inverse_b.into()],
        )?;

        let descriptor = DeclaredRelationshipDescriptor::try_from_holon(declared.into())?;

        assert!(matches!(
            descriptor.has_inverse(),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "HasInverse" && count == 2
        ));

        Ok(())
    }

    #[test]
    fn shared_relationship_accessors_work_on_declared_wrapper() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-type-for-shared-accessor",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let source_type = new_descriptor_holon(&context, "source-type", "SourceType", "Holon")?;
        let mut concrete = new_descriptor_holon(
            &context,
            "declared-with-source",
            "DeclaredWithSource",
            "Relationship",
        )?;
        concrete
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        concrete
            .add_related_holons(CoreRelationshipTypeName::SourceType, vec![source_type.into()])?;

        let descriptor = DeclaredRelationshipDescriptor::try_from_holon(concrete.into())?;

        assert_eq!(
            descriptor.source_type()?.header().type_name()?,
            MapString("SourceType".to_string())
        );

        Ok(())
    }
}
