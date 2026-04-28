use crate::descriptors::{
    accessor_helpers, DeclaredRelationshipDescriptor, Descriptor, HolonDescriptor,
    InverseRelationshipDescriptor, TypeHeader,
};
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::{HolonError, RelationshipName};

/// Runtime wrapper for relationship descriptors.
///
/// Relationship-specific structural and inverse-link behavior will accumulate
/// here in later phases while the wrapper itself stays just a typed view.
pub struct RelationshipDescriptor {
    holon: HolonReference,
}

impl RelationshipDescriptor {
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
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::{MapEnumValue, MapString};
    use core_types::HolonError;
    use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

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
            .with_property_value(CorePropertyTypeName::MinCardinality, 1_i64)?
            .with_property_value(CorePropertyTypeName::MaxCardinality, 3_i64)?
            .with_property_value(CorePropertyTypeName::DeletionSemantic, "Block")?;
        holon.add_related_holons(CoreRelationshipTypeName::SourceType, vec![source_type.into()])?;
        holon.add_related_holons(CoreRelationshipTypeName::TargetType, vec![target_type.into()])?;

        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(descriptor.is_definitional()?);
        assert!(!descriptor.is_ordered()?);
        assert!(!descriptor.allows_duplicates()?);
        assert_eq!(descriptor.min_cardinality()?, 1);
        assert_eq!(descriptor.max_cardinality()?, 3);
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
    fn required_singular_navigation_errors_when_multiple_targets_exist() -> Result<(), HolonError> {
        let context = build_context();
        let source_a = new_descriptor_holon(&context, "source-a", "SourceA", "Holon")?;
        let source_b = new_descriptor_holon(&context, "source-b", "SourceB", "Holon")?;
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

        let descriptor = RelationshipDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.source_type(),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "SourceType" && count == 2
        ));

        Ok(())
    }

    #[test]
    fn narrowing_convenience_methods_validate_subtype_kind() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-type-for-narrowing",
            "DeclaredRelationshipType",
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-type-for-narrowing",
            "InverseRelationshipType",
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
