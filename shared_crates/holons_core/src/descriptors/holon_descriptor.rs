use std::collections::HashSet;

use crate::descriptors::{
    accessor_helpers, inheritance::flatten_related_members, CommandDescriptor, DanceDescriptor,
    Descriptor, InverseRelationshipDescriptor, PropertyDescriptor, RelationshipDescriptor,
    TypeHeader,
};
use crate::reference_layer::HolonReference;
use core_types::{HolonError, PropertyName};
use type_names::{
    CorePropertyTypeName, CoreRelationshipTypeName, ToCommandName, ToDanceName, ToPropertyName,
    ToRelationshipName,
};

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

    /// Returns effective command descriptors across this descriptor's inheritance chain.
    pub fn afforded_commands(&self) -> Result<Vec<CommandDescriptor>, HolonError> {
        flatten_related_members(&self.holon, CoreRelationshipTypeName::AffordsCommand)
            .map(|members| members.into_iter().map(CommandDescriptor::from_holon).collect())
    }

    /// Returns effective dance descriptors across this descriptor's inheritance chain.
    pub fn afforded_dances(&self) -> Result<Vec<DanceDescriptor>, HolonError> {
        flatten_related_members(&self.holon, CoreRelationshipTypeName::Affords)
            .map(|members| members.into_iter().map(DanceDescriptor::from_holon).collect())
    }

    /// Returns effective property type descriptors across this descriptor's inheritance chain.
    pub fn properties(&self) -> Result<Vec<PropertyDescriptor>, HolonError> {
        self.flatten_property_descriptors(CoreRelationshipTypeName::Properties)
    }

    /// Finds an effective instance property by property type identity.
    pub fn get_property_by_name(
        &self,
        name: impl ToPropertyName,
    ) -> Result<PropertyDescriptor, HolonError> {
        let requested_name = name.to_property_name();
        let requested = requested_name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        for descriptor in self.instance_properties()? {
            let declaration_name = PropertyName(descriptor.header().type_name()?);
            let declaration_label = declaration_name.to_string();
            if !seen.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "property".to_string(),
                    name: declaration_label,
                    descriptor: accessor_helpers::descriptor_label(&self.holon),
                });
            }
            if declaration_name == requested_name {
                found = Some(descriptor);
            }
        }

        found.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
            kind: "property".to_string(),
            name: requested,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        })
    }

    /// Finds an effective command affordance by command descriptor type name.
    pub fn get_command_by_name(
        &self,
        name: impl ToCommandName,
    ) -> Result<CommandDescriptor, HolonError> {
        let requested_name = name.to_command_name();
        let requested = requested_name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        for descriptor in self.afforded_commands()? {
            let declaration_name = descriptor.command_name()?;
            let declaration_label = declaration_name.to_string();
            if !seen.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "command".to_string(),
                    name: declaration_label,
                    descriptor: accessor_helpers::descriptor_label(&self.holon),
                });
            }
            if declaration_name == requested_name {
                found = Some(descriptor);
            }
        }

        found.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
            kind: "command".to_string(),
            name: requested,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        })
    }

    /// Finds an effective dance affordance by dance descriptor type name.
    pub fn get_dance_by_name(&self, name: impl ToDanceName) -> Result<DanceDescriptor, HolonError> {
        let requested_name = name.to_dance_name();
        let requested = requested_name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        for descriptor in self.afforded_dances()? {
            let declaration_name = descriptor.dance_name()?;
            let declaration_label = declaration_name.to_string();
            if !seen.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "dance".to_string(),
                    name: declaration_label,
                    descriptor: accessor_helpers::descriptor_label(&self.holon),
                });
            }
            if declaration_name == requested_name {
                found = Some(descriptor);
            }
        }

        found.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
            kind: "dance".to_string(),
            name: requested,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        })
    }

    /// Finds an effective instance relationship by base relationship name.
    pub fn get_relationship_by_name(
        &self,
        name: impl ToRelationshipName,
    ) -> Result<RelationshipDescriptor, HolonError> {
        let requested_name = name.to_relationship_name();
        let requested = requested_name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        for descriptor in self.instance_relationships()? {
            let declaration_name = descriptor.base_relationship_name()?;
            let declaration_label = declaration_name.to_string();
            if !seen.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "relationship".to_string(),
                    name: declaration_label,
                    descriptor: accessor_helpers::descriptor_label(&self.holon),
                });
            }
            if declaration_name == requested_name {
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
        declared_name: impl ToRelationshipName,
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
    use type_names::{
        CommandName, CoreCommandTypeName, CoreHolonTypeName, CorePropertyTypeName,
        CoreRelationshipTypeName, DanceName,
    };

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

    fn command_names(commands: Vec<CommandDescriptor>) -> Result<Vec<CommandName>, HolonError> {
        commands.into_iter().map(|command| command.command_name()).collect()
    }

    fn dance_names(dances: Vec<DanceDescriptor>) -> Result<Vec<DanceName>, HolonError> {
        dances.into_iter().map(|dance| dance.dance_name()).collect()
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
    fn afforded_commands_returns_empty_when_no_affords_command_edges_present(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let holon_type = new_descriptor_holon(&context, "no-command-owner", "BookType")?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert!(descriptor.afforded_commands()?.is_empty());

        Ok(())
    }

    #[test]
    fn afforded_commands_returns_self_first_then_inherited() -> Result<(), HolonError> {
        let context = build_context();
        let inherited_command =
            new_descriptor_holon(&context, "inherited-command", "BeginTransaction")?;
        let local_command = new_descriptor_holon(&context, "local-command", "Commit")?;
        let mut parent = new_descriptor_holon(&context, "command-parent", "ParentType")?;
        let mut leaf = new_descriptor_holon(&context, "command-leaf", "LeafType")?;

        parent.add_related_holons(
            CoreRelationshipTypeName::AffordsCommand,
            vec![inherited_command.into()],
        )?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
        leaf.add_related_holons(
            CoreRelationshipTypeName::AffordsCommand,
            vec![local_command.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());

        assert_eq!(
            command_names(descriptor.afforded_commands()?)?,
            vec![
                CommandName(MapString("Commit".to_string())),
                CommandName(MapString("BeginTransaction".to_string())),
            ]
        );

        Ok(())
    }

    #[test]
    fn afforded_commands_flattens_through_multi_step_extends_chain() -> Result<(), HolonError> {
        let context = build_context();
        let root_command = new_descriptor_holon(&context, "root-command", "BeginTransaction")?;
        let middle_command = new_descriptor_holon(&context, "middle-command", "CloneHolon")?;
        let leaf_command = new_descriptor_holon(&context, "leaf-command", "Commit")?;
        let mut root = new_descriptor_holon(&context, "command-root", "RootType")?;
        let mut middle = new_descriptor_holon(&context, "command-middle", "MiddleType")?;
        let mut leaf = new_descriptor_holon(&context, "command-chain-leaf", "LeafType")?;

        root.add_related_holons(
            CoreRelationshipTypeName::AffordsCommand,
            vec![root_command.into()],
        )?;
        middle.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        middle.add_related_holons(
            CoreRelationshipTypeName::AffordsCommand,
            vec![middle_command.into()],
        )?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![middle.into()])?;
        leaf.add_related_holons(
            CoreRelationshipTypeName::AffordsCommand,
            vec![leaf_command.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());

        assert_eq!(
            command_names(descriptor.afforded_commands()?)?,
            vec![
                CommandName(MapString("Commit".to_string())),
                CommandName(MapString("CloneHolon".to_string())),
                CommandName(MapString("BeginTransaction".to_string())),
            ]
        );

        Ok(())
    }

    #[test]
    fn get_command_by_name_matches_on_shared_type_name() -> Result<(), HolonError> {
        let context = build_context();
        let command = new_descriptor_holon(&context, "commit-command-affordance", "Commit")?;
        let mut holon_type = new_descriptor_holon(&context, "command-owner", "TransactionType")?;

        holon_type
            .add_related_holons(CoreRelationshipTypeName::AffordsCommand, vec![command.into()])?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert_eq!(
            descriptor.get_command_by_name(CoreCommandTypeName::Commit)?.command_name()?,
            CommandName(MapString("Commit".to_string()))
        );

        Ok(())
    }

    #[test]
    fn afforded_dances_returns_self_first_then_inherited() -> Result<(), HolonError> {
        let context = build_context();
        let inherited_dance = new_descriptor_holon(&context, "inherited-dance", "Query")?;
        let local_dance = new_descriptor_holon(&context, "local-dance", "Dance")?;
        let mut parent = new_descriptor_holon(&context, "dance-parent", "ParentType")?;
        let mut leaf = new_descriptor_holon(&context, "dance-leaf", "LeafType")?;

        parent
            .add_related_holons(CoreRelationshipTypeName::Affords, vec![inherited_dance.into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Affords, vec![local_dance.into()])?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());

        assert_eq!(
            dance_names(descriptor.afforded_dances()?)?,
            vec![
                DanceName(MapString("Dance".to_string())),
                DanceName(MapString("Query".to_string())),
            ]
        );

        Ok(())
    }

    #[test]
    fn afforded_dances_flattens_through_multi_step_extends_chain() -> Result<(), HolonError> {
        let context = build_context();
        let root_dance = new_descriptor_holon(&context, "root-dance", "Dance")?;
        let middle_dance = new_descriptor_holon(&context, "middle-dance", "Query")?;
        let leaf_dance = new_descriptor_holon(&context, "leaf-dance", "LoadHolons")?;
        let mut root = new_descriptor_holon(&context, "dance-root", "RootType")?;
        let mut middle = new_descriptor_holon(&context, "dance-middle", "MiddleType")?;
        let mut leaf = new_descriptor_holon(&context, "dance-chain-leaf", "LeafType")?;

        root.add_related_holons(CoreRelationshipTypeName::Affords, vec![root_dance.into()])?;
        middle.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        middle.add_related_holons(CoreRelationshipTypeName::Affords, vec![middle_dance.into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![middle.into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Affords, vec![leaf_dance.into()])?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());

        assert_eq!(
            dance_names(descriptor.afforded_dances()?)?,
            vec![
                DanceName(MapString("LoadHolons".to_string())),
                DanceName(MapString("Query".to_string())),
                DanceName(MapString("Dance".to_string())),
            ]
        );

        Ok(())
    }

    #[test]
    fn get_dance_by_name_matches_on_shared_type_name() -> Result<(), HolonError> {
        let context = build_context();
        let dance = new_descriptor_holon(&context, "dance-affordance", "Query")?;
        let mut holon_type = new_descriptor_holon(&context, "dance-owner", "TransactionType")?;

        holon_type.add_related_holons(CoreRelationshipTypeName::Affords, vec![dance.into()])?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert_eq!(
            descriptor.get_dance_by_name("query")?.dance_name()?,
            DanceName(MapString("Query".to_string()))
        );

        Ok(())
    }

    #[test]
    fn get_dance_by_name_errors_when_not_found() -> Result<(), HolonError> {
        let context = build_context();
        let dance = new_descriptor_holon(&context, "query-dance-affordance", "Query")?;
        let mut holon_type = new_descriptor_holon(&context, "missing-dance-owner", "BookType")?;

        holon_type.add_related_holons(CoreRelationshipTypeName::Affords, vec![dance.into()])?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert!(matches!(
            descriptor.get_dance_by_name("Commit"),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "dance" && name == "Commit"
        ));

        Ok(())
    }

    #[test]
    fn get_dance_by_name_detects_duplicate_inherited_declarations() -> Result<(), HolonError> {
        let context = build_context();
        let duplicate_root = new_descriptor_holon(&context, "duplicate-root-dance", "Query")?;
        let duplicate_leaf = new_descriptor_holon(&context, "duplicate-leaf-dance", "Query")?;
        let mut root = new_descriptor_holon(&context, "duplicate-dance-root", "RootType")?;
        let mut leaf = new_descriptor_holon(&context, "duplicate-dance-leaf", "LeafType")?;

        root.add_related_holons(CoreRelationshipTypeName::Affords, vec![duplicate_root.into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Affords, vec![duplicate_leaf.into()])?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());

        assert!(matches!(
            descriptor.get_dance_by_name("Query"),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "dance" && name == "Query"
        ));

        Ok(())
    }

    #[test]
    fn afforded_dances_errors_on_cyclic_extends() -> Result<(), HolonError> {
        let context = build_context();
        let mut descriptor_a = new_descriptor_holon(&context, "dance-cycle-a", "CycleA")?;
        let mut descriptor_b = new_descriptor_holon(&context, "dance-cycle-b", "CycleB")?;

        descriptor_a.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![descriptor_b.clone().into()],
        )?;
        descriptor_b.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![descriptor_a.clone().into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(descriptor_a.into());

        assert!(matches!(descriptor.afforded_dances(), Err(HolonError::CyclicExtends { .. })));

        Ok(())
    }

    #[test]
    fn afforded_dances_errors_when_multiple_extends_are_declared() -> Result<(), HolonError> {
        let context = build_context();
        let parent_a = new_descriptor_holon(&context, "dance-parent-a", "ParentA")?;
        let parent_b = new_descriptor_holon(&context, "dance-parent-b", "ParentB")?;
        let mut child = new_descriptor_holon(&context, "dance-child", "ChildType")?;

        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![parent_a.into(), parent_b.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.afforded_dances(),
            Err(HolonError::MultipleExtends { count, .. }) if count == 2
        ));

        Ok(())
    }

    #[test]
    fn get_property_by_name_accepts_conversion_trait_inputs() -> Result<(), HolonError> {
        let context = build_context();
        let property = new_descriptor_holon(&context, "display-name-property", "DisplayName")?;
        let mut holon_type = new_descriptor_holon(&context, "property-owner", "BookType")?;
        holon_type.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![property.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert_eq!(
            descriptor.get_property_by_name("display_name")?.header().type_name()?,
            MapString("DisplayName".to_string())
        );

        Ok(())
    }

    #[test]
    fn get_relationship_by_name_accepts_conversion_trait_inputs() -> Result<(), HolonError> {
        let context = build_context();
        let relationship = new_descriptor_holon(&context, "relationship-match", "AuthoredBy")?;
        let mut holon_type = new_descriptor_holon(&context, "relationship-owner", "BookType")?;
        holon_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![relationship.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert_eq!(
            descriptor.get_relationship_by_name("authored_by")?.base_relationship_name()?,
            RelationshipName(MapString("AuthoredBy".to_string()))
        );

        Ok(())
    }

    #[test]
    fn get_command_by_name_errors_when_not_found() -> Result<(), HolonError> {
        let context = build_context();
        let command = new_descriptor_holon(&context, "query-command-affordance", "Query")?;
        let mut holon_type = new_descriptor_holon(&context, "missing-command-owner", "BookType")?;

        holon_type
            .add_related_holons(CoreRelationshipTypeName::AffordsCommand, vec![command.into()])?;

        let descriptor = HolonDescriptor::from_holon(holon_type.into());

        assert!(matches!(
            descriptor.get_command_by_name(MapString("Commit".to_string())),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "command" && name == "Commit"
        ));

        Ok(())
    }

    #[test]
    fn get_command_by_name_detects_duplicate_inherited_declarations() -> Result<(), HolonError> {
        let context = build_context();
        let duplicate_root = new_descriptor_holon(&context, "duplicate-root-command", "Commit")?;
        let duplicate_leaf = new_descriptor_holon(&context, "duplicate-leaf-command", "Commit")?;
        let mut root = new_descriptor_holon(&context, "duplicate-command-root", "RootType")?;
        let mut leaf = new_descriptor_holon(&context, "duplicate-command-leaf", "LeafType")?;

        root.add_related_holons(
            CoreRelationshipTypeName::AffordsCommand,
            vec![duplicate_root.into()],
        )?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        leaf.add_related_holons(
            CoreRelationshipTypeName::AffordsCommand,
            vec![duplicate_leaf.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(leaf.into());

        assert!(matches!(
            descriptor.get_command_by_name(MapString("Commit".to_string())),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "command" && name == "Commit"
        ));

        Ok(())
    }

    #[test]
    fn afforded_commands_errors_on_cyclic_extends() -> Result<(), HolonError> {
        let context = build_context();
        let mut descriptor_a = new_descriptor_holon(&context, "command-cycle-a", "CycleA")?;
        let mut descriptor_b = new_descriptor_holon(&context, "command-cycle-b", "CycleB")?;

        descriptor_a.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![descriptor_b.clone().into()],
        )?;
        descriptor_b.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![descriptor_a.clone().into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(descriptor_a.into());

        assert!(matches!(descriptor.afforded_commands(), Err(HolonError::CyclicExtends { .. })));

        Ok(())
    }

    #[test]
    fn afforded_commands_errors_when_multiple_extends_are_declared() -> Result<(), HolonError> {
        let context = build_context();
        let parent_a = new_descriptor_holon(&context, "command-parent-a", "ParentA")?;
        let parent_b = new_descriptor_holon(&context, "command-parent-b", "ParentB")?;
        let mut child = new_descriptor_holon(&context, "command-child", "ChildType")?;

        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![parent_a.into(), parent_b.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.afforded_commands(),
            Err(HolonError::MultipleExtends { count, .. }) if count == 2
        ));

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
