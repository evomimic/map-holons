use std::collections::HashSet;

use crate::descriptors::{
    accessor_helpers, inheritance::flatten_related_members, Descriptor,
    InverseRelationshipDescriptor, PropertyDescriptor, RelationshipDescriptor, TypeHeader,
};
use crate::reference_layer::HolonReference;
use core_types::{HolonError, PropertyName, RelationshipName};
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

/// Runtime wrapper for holon-type descriptors.
///
/// This is the main descriptor surface that callers will reach from ordinary
/// holon instances via `ReadableHolon::holon_descriptor()`.
pub struct HolonDescriptor {
    holon: HolonReference,
}

impl HolonDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns whether instances may carry properties beyond the descriptor declaration.
    pub fn allows_additional_properties(&self) -> Result<bool, HolonError> {
        accessor_helpers::require_bool(
            &self.holon,
            CorePropertyTypeName::AllowsAdditionalProperties,
        )
    }

    /// Returns whether instances may carry relationships beyond the descriptor declaration.
    pub fn allows_additional_relationships(&self) -> Result<bool, HolonError> {
        accessor_helpers::require_bool(
            &self.holon,
            CorePropertyTypeName::AllowsAdditionalRelationships,
        )
    }

    /// Returns effective instance property descriptors across this descriptor's inheritance chain.
    pub fn instance_properties(&self) -> Result<Vec<PropertyDescriptor>, HolonError> {
        self.flatten_property_descriptors(CoreRelationshipTypeName::InstanceProperties)
    }

    /// Returns effective instance relationship descriptors across this descriptor's inheritance chain.
    pub fn instance_relationships(&self) -> Result<Vec<RelationshipDescriptor>, HolonError> {
        flatten_related_members(&self.holon, CoreRelationshipTypeName::InstanceRelationships)
            .map(|members| members.into_iter().map(RelationshipDescriptor::from_holon).collect())
    }

    /// Returns effective property type descriptors across this descriptor's inheritance chain.
    pub fn properties(&self) -> Result<Vec<PropertyDescriptor>, HolonError> {
        self.flatten_property_descriptors(CoreRelationshipTypeName::Properties)
    }

    /// Finds an effective instance property by property type identity.
    pub fn get_property_by_name(
        &self,
        name: PropertyName,
    ) -> Result<PropertyDescriptor, HolonError> {
        let requested = name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        for descriptor in self.instance_properties()? {
            let declaration_name = descriptor.header().type_name()?.to_string();
            if !seen.insert(declaration_name.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "property".to_string(),
                    name: declaration_name,
                    descriptor: accessor_helpers::descriptor_label(&self.holon),
                });
            }
            if declaration_name == requested {
                found = Some(descriptor);
            }
        }

        found.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
            kind: "property".to_string(),
            name: requested,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        })
    }

    /// Finds an effective instance relationship by base relationship name.
    pub fn get_relationship_by_name(
        &self,
        name: RelationshipName,
    ) -> Result<RelationshipDescriptor, HolonError> {
        let requested = name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        for descriptor in self.instance_relationships()? {
            let declaration_name = descriptor.base_relationship_name()?.to_string();
            if !seen.insert(declaration_name.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "relationship".to_string(),
                    name: declaration_name,
                    descriptor: accessor_helpers::descriptor_label(&self.holon),
                });
            }
            if declaration_name == requested {
                found = Some(descriptor);
            }
        }

        found.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
            kind: "relationship".to_string(),
            name: requested,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        })
    }

    /// Finds the inverse descriptor for an effective declared relationship name.
    pub fn get_inverse_relationship_by_name(
        &self,
        declared_name: RelationshipName,
    ) -> Result<InverseRelationshipDescriptor, HolonError> {
        let declared = self
            .get_relationship_by_name(declared_name)?
            .try_into_declared_relationship_descriptor()?;

        declared.has_inverse()?.ok_or_else(|| HolonError::MissingRequiredRelationship {
            relationship: "HasInverse".to_string(),
            descriptor: accessor_helpers::descriptor_label(declared.holon()),
        })
    }

    fn flatten_property_descriptors(
        &self,
        relationship_name: CoreRelationshipTypeName,
    ) -> Result<Vec<PropertyDescriptor>, HolonError> {
        flatten_related_members(&self.holon, relationship_name)
            .map(|members| members.into_iter().map(PropertyDescriptor::from_holon).collect())
    }
}

impl From<HolonReference> for HolonDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for HolonDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support::{build_context, core_holon_type_name, new_test_holon};
    use crate::reference_layer::{ReadableHolon, WritableHolon};
    use crate::TransientReference;
    use base_types::MapString;
    use core_types::{HolonError, PropertyName, RelationshipName};
    use std::sync::Arc;
    use type_names::{CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

    fn new_descriptor_holon(
        context: &Arc<TransactionContext>,
        key: &str,
        type_name: &str,
    ) -> Result<TransientReference, HolonError> {
        // Descriptor tests only need the shared header surface in this phase.
        let mut descriptor = new_test_holon(context, key)?;
        descriptor
            .with_property_value(CorePropertyTypeName::TypeName, type_name)?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;
        Ok(descriptor)
    }

    fn assert_is_descriptor<T: Descriptor>(descriptor: &T) {
        // Compile-time trait membership plus one trivial runtime use.
        let _ = descriptor.holon().reference_id_string();
    }

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon =
            HolonReference::from(&new_descriptor_holon(&context, "holon-descriptor", "HolonType")?);

        let descriptor = HolonDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("HolonType".to_string()));
        assert_is_descriptor(&descriptor);

        Ok(())
    }

    #[test]
    fn holon_descriptor_resolves_for_transient_source() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor =
            new_descriptor_holon(&context, "descriptor-transient", "TransientDescriptor")?;
        let mut source = new_test_holon(&context, "source-transient")?;
        source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![descriptor.clone().into()],
        )?;

        let resolved = source.holon_descriptor()?;

        assert_eq!(resolved.header().type_name()?, MapString("TransientDescriptor".to_string()));
        assert_eq!(resolved.holon(), &HolonReference::from(&descriptor));
        assert_is_descriptor(&resolved);

        Ok(())
    }

    #[test]
    fn holon_descriptor_resolves_for_staged_source() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = new_descriptor_holon(&context, "descriptor-staged", "StagedDescriptor")?;
        let staged_descriptor = context.mutation().stage_new_holon(descriptor)?;
        let source = new_test_holon(&context, "source-staged")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        staged_source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![staged_descriptor.into()],
        )?;

        let resolved = staged_source.holon_descriptor()?;

        assert_eq!(resolved.header().type_name()?, MapString("StagedDescriptor".to_string()));
        assert_is_descriptor(&resolved);

        Ok(())
    }

    #[test]
    fn holon_descriptor_errors_when_described_by_missing() -> Result<(), HolonError> {
        let context = build_context();
        let source = new_test_holon(&context, "missing-descriptor")?;

        assert!(matches!(source.holon_descriptor(), Err(HolonError::MissingDescribedBy { .. })));

        Ok(())
    }

    #[test]
    fn holon_descriptor_errors_when_multiple_described_by_present() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor_a = new_descriptor_holon(&context, "descriptor-a", "DescriptorA")?;
        let descriptor_b = new_descriptor_holon(&context, "descriptor-b", "DescriptorB")?;
        let mut source = new_test_holon(&context, "multiple-descriptor-source")?;
        source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![descriptor_a.into(), descriptor_b.into()],
        )?;

        assert!(matches!(
            source.holon_descriptor(),
            Err(HolonError::MultipleDescribedBy { count, .. }) if count == 2
        ));

        Ok(())
    }

    #[test]
    fn structural_flags_return_required_boolean_values() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(&context, "structural-flags", "BookType")?;
        holon
            .with_property_value(CorePropertyTypeName::AllowsAdditionalProperties, true)?
            .with_property_value(CorePropertyTypeName::AllowsAdditionalRelationships, false)?;

        let descriptor = HolonDescriptor::from_holon(holon.into());

        assert!(descriptor.allows_additional_properties()?);
        assert!(!descriptor.allows_additional_relationships()?);

        Ok(())
    }

    #[test]
    fn structural_flags_error_when_required_boolean_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(&context, "missing-structural-flag", "BookType")?;
        holon.with_property_value(CorePropertyTypeName::AllowsAdditionalRelationships, true)?;

        let descriptor = HolonDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.allows_additional_properties(),
            Err(HolonError::EmptyField(field)) if field == "AllowsAdditionalProperties"
        ));

        Ok(())
    }

    #[test]
    fn structural_flags_error_when_relationship_flag_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon =
            new_descriptor_holon(&context, "missing-relationship-structural-flag", "BookType")?;
        holon.with_property_value(CorePropertyTypeName::AllowsAdditionalProperties, true)?;

        let descriptor = HolonDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.allows_additional_relationships(),
            Err(HolonError::EmptyField(field)) if field == "AllowsAdditionalRelationships"
        ));

        Ok(())
    }

    #[test]
    fn structural_flags_error_when_required_boolean_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(&context, "wrong-type-structural-flag", "BookType")?;
        holon.with_property_value(
            CorePropertyTypeName::AllowsAdditionalProperties,
            "not-a-boolean",
        )?;

        let descriptor = HolonDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.allows_additional_properties(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Boolean"
        ));

        Ok(())
    }

    #[test]
    fn flattened_plural_accessors_preserve_self_first_inheritance_order() -> Result<(), HolonError>
    {
        let context = build_context();
        let prop_root = new_descriptor_holon(&context, "prop-root", "RootProperty")?;
        let prop_middle = new_descriptor_holon(&context, "prop-middle", "MiddleProperty")?;
        let prop_leaf = new_descriptor_holon(&context, "prop-leaf", "LeafProperty")?;
        let property_type = new_descriptor_holon(&context, "property-type", "PropertyType")?;
        let rel_root = new_descriptor_holon(&context, "rel-root", "RootRelationship")?;
        let rel_leaf = new_descriptor_holon(&context, "rel-leaf", "LeafRelationship")?;
        let mut root = new_descriptor_holon(&context, "root-type", "RootType")?;
        let mut middle = new_descriptor_holon(&context, "middle-type", "MiddleType")?;
        let mut leaf = new_descriptor_holon(&context, "leaf-type", "LeafType")?;

        root.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![prop_root.clone().into()],
        )?;
        root.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![rel_root.clone().into()],
        )?;
        root.add_related_holons(CoreRelationshipTypeName::Properties, vec![property_type.into()])?;
        middle.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        middle.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![prop_middle.clone().into()],
        )?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![middle.into()])?;
        leaf.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![prop_leaf.clone().into()],
        )?;
        leaf.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![rel_leaf.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());
        let property_names = descriptor
            .instance_properties()?
            .into_iter()
            .map(|property| property.header().type_name())
            .collect::<Result<Vec<_>, _>>()?;
        let relationship_names = descriptor
            .instance_relationships()?
            .into_iter()
            .map(|relationship| relationship.header().type_name())
            .collect::<Result<Vec<_>, _>>()?;
        let properties_names = descriptor
            .properties()?
            .into_iter()
            .map(|property| property.header().type_name())
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(
            property_names,
            vec![
                MapString("LeafProperty".to_string()),
                MapString("MiddleProperty".to_string()),
                MapString("RootProperty".to_string()),
            ]
        );
        assert_eq!(
            relationship_names,
            vec![
                MapString("LeafRelationship".to_string()),
                MapString("RootRelationship".to_string()),
            ]
        );
        assert_eq!(properties_names, vec![MapString("PropertyType".to_string())]);

        Ok(())
    }

    #[test]
    fn get_property_by_name_detects_duplicate_inherited_declarations() -> Result<(), HolonError> {
        let context = build_context();
        let duplicate_root = new_descriptor_holon(&context, "duplicate-root", "DuplicateProperty")?;
        let duplicate_leaf = new_descriptor_holon(&context, "duplicate-leaf", "DuplicateProperty")?;
        let mut root = new_descriptor_holon(&context, "duplicate-root-type", "RootType")?;
        let mut leaf = new_descriptor_holon(&context, "duplicate-leaf-type", "LeafType")?;

        root.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![duplicate_root.into()],
        )?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        leaf.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![duplicate_leaf.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());

        assert!(matches!(
            descriptor.get_property_by_name(PropertyName(MapString("DuplicateProperty".to_string()))),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "property" && name == "DuplicateProperty"
        ));

        Ok(())
    }

    #[test]
    fn get_relationship_by_name_detects_duplicate_inherited_declarations() -> Result<(), HolonError>
    {
        let context = build_context();
        let duplicate_root =
            new_descriptor_holon(&context, "duplicate-root-rel", "DuplicateRelationship")?;
        let duplicate_leaf =
            new_descriptor_holon(&context, "duplicate-leaf-rel", "DuplicateRelationship")?;
        let mut root = new_descriptor_holon(&context, "duplicate-root-rel-type", "RootType")?;
        let mut leaf = new_descriptor_holon(&context, "duplicate-leaf-rel-type", "LeafType")?;

        root.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![duplicate_root.into()],
        )?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        leaf.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![duplicate_leaf.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());

        assert!(matches!(
            descriptor.get_relationship_by_name(RelationshipName(MapString(
                "DuplicateRelationship".to_string()
            ))),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "relationship" && name == "DuplicateRelationship"
        ));

        Ok(())
    }

    #[test]
    fn get_relationship_by_name_returns_match_and_reports_missing() -> Result<(), HolonError> {
        let context = build_context();
        let relationship = new_descriptor_holon(&context, "relationship-match", "AuthoredBy")?;
        let mut holon_type = new_descriptor_holon(&context, "relationship-owner", "BookType")?;
        holon_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![relationship.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert_eq!(
            descriptor
                .get_relationship_by_name(RelationshipName(MapString("AuthoredBy".to_string())))?
                .base_relationship_name()?
                .to_string(),
            "AuthoredBy"
        );
        assert!(matches!(
            descriptor.get_relationship_by_name(RelationshipName(MapString("Missing".to_string()))),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "Missing"
        ));

        Ok(())
    }

    #[test]
    fn get_inverse_relationship_by_name_follows_declared_inverse() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
        )?;
        let inverse_type = new_descriptor_holon(
            &context,
            "inverse-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
        )?;
        let mut declared = new_descriptor_holon(&context, "authored-by", "AuthoredBy")?;
        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        let mut inverse = new_descriptor_holon(&context, "books-authored", "BooksAuthored")?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        declared.add_related_holons(CoreRelationshipTypeName::HasInverse, vec![inverse.into()])?;
        let mut holon_type = new_descriptor_holon(&context, "book-type-with-inverse", "BookType")?;
        holon_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![declared.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert_eq!(
            descriptor
                .get_inverse_relationship_by_name(RelationshipName(MapString(
                    "AuthoredBy".to_string()
                )))?
                .header()
                .type_name()?,
            MapString("BooksAuthored".to_string())
        );

        Ok(())
    }

    #[test]
    fn get_inverse_relationship_by_name_errors_when_inverse_missing() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "declared-type-missing-inverse",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
        )?;
        let mut declared =
            new_descriptor_holon(&context, "declared-no-inverse", "DeclaredNoInverse")?;
        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        let mut holon_type =
            new_descriptor_holon(&context, "book-type-missing-inverse", "BookType")?;
        holon_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![declared.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert!(matches!(
            descriptor.get_inverse_relationship_by_name(RelationshipName(MapString(
                "DeclaredNoInverse".to_string()
            ))),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "HasInverse"
        ));

        Ok(())
    }
}
