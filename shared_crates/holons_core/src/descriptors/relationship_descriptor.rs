use crate::descriptors::{accessor_helpers, Descriptor, HolonDescriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::{HolonError, RelationshipName};
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

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
        accessor_helpers::require_bool(&self.holon, CorePropertyTypeName::IsDefinitional)
    }

    /// Returns whether related members have schema-significant order.
    pub fn is_ordered(&self) -> Result<bool, HolonError> {
        accessor_helpers::require_bool(&self.holon, CorePropertyTypeName::IsOrdered)
    }

    /// Returns whether repeated target references are allowed.
    pub fn allows_duplicates(&self) -> Result<bool, HolonError> {
        accessor_helpers::require_bool(&self.holon, CorePropertyTypeName::AllowsDuplicates)
    }

    /// Returns the minimum number of targets permitted by this relationship.
    pub fn min_cardinality(&self) -> Result<i64, HolonError> {
        accessor_helpers::require_integer(&self.holon, CorePropertyTypeName::MinCardinality)
    }

    /// Returns the maximum number of targets permitted by this relationship.
    pub fn max_cardinality(&self) -> Result<i64, HolonError> {
        accessor_helpers::require_integer(&self.holon, CorePropertyTypeName::MaxCardinality)
    }

    /// Returns the optional deletion semantic declared by this relationship, when populated.
    pub fn deletion_semantic(&self) -> Result<Option<MapString>, HolonError> {
        accessor_helpers::optional_string(&self.holon, CorePropertyTypeName::DeletionSemantic)
    }

    /// Returns this descriptor's base relationship name.
    pub fn base_relationship_name(&self) -> Result<RelationshipName, HolonError> {
        Ok(RelationshipName(self.header().type_name()?))
    }

    /// Returns the source holon descriptor reached through the required `SourceType` relationship.
    pub fn source_type(&self) -> Result<HolonDescriptor, HolonError> {
        let source_type = accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::SourceType,
        )?;
        Ok(HolonDescriptor::from_holon(source_type))
    }

    /// Returns the target holon descriptor reached through the required `TargetType` relationship.
    pub fn target_type(&self) -> Result<HolonDescriptor, HolonError> {
        let target_type = accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::TargetType,
        )?;
        Ok(HolonDescriptor::from_holon(target_type))
    }

    /// Returns the full `(Source)-[Base]->(Target)` relationship name.
    pub fn full_relationship_name(&self) -> Result<MapString, HolonError> {
        let source_name = self.source_type()?.header().type_name()?;
        let base_name = self.base_relationship_name()?;
        let target_name = self.target_type()?.header().type_name()?;

        Ok(MapString(format!("({source_name})-[{base_name}]->({target_name})")))
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
}
