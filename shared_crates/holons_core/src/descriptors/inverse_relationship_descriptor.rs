use crate::descriptors::{
    accessor_helpers, DeclaredRelationshipDescriptor, Descriptor, HolonDescriptor, TypeHeader,
};
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::{HolonError, RelationshipName};
use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

/// Runtime wrapper for inverse relationship descriptors.
///
/// Construction validates that the descriptor's effective `Extends` chain reaches
/// [`CoreHolonTypeName::InverseRelationshipType`].
pub struct InverseRelationshipDescriptor {
    holon: HolonReference,
}

impl InverseRelationshipDescriptor {
    /// Wraps a relationship descriptor only if it extends
    /// [`CoreHolonTypeName::InverseRelationshipType`].
    pub fn try_from_holon(holon: HolonReference) -> Result<Self, HolonError> {
        accessor_helpers::validate_extends_chain_reaches(
            &holon,
            &CoreHolonTypeName::InverseRelationshipType.as_holon_name(),
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

    /// Returns the declared relationship descriptor reached through required `InverseOf`.
    pub fn inverse_of(&self) -> Result<DeclaredRelationshipDescriptor, HolonError> {
        let declared = accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::InverseOf,
        )?;
        DeclaredRelationshipDescriptor::try_from_holon(declared)
    }
}

impl Descriptor for InverseRelationshipDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<InverseRelationshipDescriptor>();
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
    fn try_from_holon_accepts_inverse_relationship_chain() -> Result<(), HolonError> {
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

        let descriptor = InverseRelationshipDescriptor::try_from_holon(concrete.into())?;

        assert_eq!(descriptor.header().type_name()?, MapString("AuthoredBooks".to_string()));

        Ok(())
    }

    #[test]
    fn try_from_holon_rejects_wrong_relationship_kind() -> Result<(), HolonError> {
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

        assert!(matches!(
            InverseRelationshipDescriptor::try_from_holon(concrete.into()),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == core_holon_type_name(CoreHolonTypeName::InverseRelationshipType)
                    && found == "AuthoredBy"
        ));

        Ok(())
    }

    #[test]
    fn inverse_of_returns_declared_relationship() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-type-for-inverse-of",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-type-for-inverse-of",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut declared =
            new_descriptor_holon(&context, "author-of-book", "AuthorOfBook", "Relationship")?;
        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        let mut inverse =
            new_descriptor_holon(&context, "books-by-author", "BooksByAuthor", "Relationship")?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        inverse.add_related_holons(CoreRelationshipTypeName::InverseOf, vec![declared.into()])?;

        let descriptor = InverseRelationshipDescriptor::try_from_holon(inverse.into())?;

        assert_eq!(
            descriptor.inverse_of()?.header().type_name()?,
            MapString("AuthorOfBook".to_string())
        );

        Ok(())
    }

    #[test]
    fn inverse_of_errors_when_required_relationship_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-type-for-missing-inverse-of",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut inverse = new_descriptor_holon(
            &context,
            "missing-inverse-of",
            "MissingInverseOf",
            "Relationship",
        )?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;

        let descriptor = InverseRelationshipDescriptor::try_from_holon(inverse.into())?;

        assert!(matches!(
            descriptor.inverse_of(),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "InverseOf"
        ));

        Ok(())
    }
}
