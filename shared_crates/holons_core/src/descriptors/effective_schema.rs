//! Lossless, reference-oriented effective schema computation and lazy vocabulary indexes.
use std::{collections::HashMap, sync::OnceLock};

use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::{CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

use super::{
    ancestors, effective_holon_descriptor::collections, effective_relationships,
    inheritance::relationship_targets_in_lineage, ContractContributions,
    EffectiveRelationshipMember, ValidExtendsLineage,
};
use crate::{DefinitionIdentity, HolonReference, ReadableHolon};

/// One selected declaration occurrence. Position is local to its original source collection.
#[derive(Clone, Debug)]
pub struct EffectiveContribution {
    pub member: HolonReference,
    pub declared_on: HolonReference,
    pub source_collection: CoreRelationshipTypeName,
    pub local_position: usize,
}

/// Name namespaces with explicit uniqueness requirements.
#[derive(Clone, Copy, Debug)]
pub enum EffectiveVocabulary {
    Properties,
    Relationships,
    Dances,
    Commands,
}

impl EffectiveVocabulary {
    pub fn label(self) -> &'static str {
        match self {
            Self::Properties => "property",
            Self::Relationships => "relationship",
            Self::Dances => "dance",
            Self::Commands => "command",
        }
    }
}

/// A complete index retains conflicts and unnamed declarations as diagnostic evidence.
#[derive(Debug, Default)]
pub struct EffectiveNameIndex {
    pub entries: HashMap<MapString, Vec<EffectiveContribution>>,
    pub unnamed: Vec<EffectiveContribution>,
}

pub(super) struct EffectiveCollection {
    pub source: CoreRelationshipTypeName,
    pub result: CoreRelationshipTypeName,
    pub occurrences: Vec<EffectiveContribution>,
}

/// A schema snapshot with no transient allocations and no source-schema validation.
/// References remain bound handles. Discard this snapshot after a contributing schema edit.
/// Navigation and semantic-name indexes are computed at most once, when requested.
pub struct ComputedEffectiveSchema {
    pub target: HolonReference,
    pub lineage: Vec<HolonReference>,
    pub(super) collections: Vec<EffectiveCollection>,
    navigation: OnceLock<Result<Vec<EffectiveContribution>, HolonError>>,
    properties: OnceLock<Result<EffectiveNameIndex, HolonError>>,
    relationships: OnceLock<Result<EffectiveNameIndex, HolonError>>,
    dances: OnceLock<Result<EffectiveNameIndex, HolonError>>,
    commands: OnceLock<Result<EffectiveNameIndex, HolonError>>,
}

impl ComputedEffectiveSchema {
    /// Restores occurrence evidence from a prepared saved-schema result without Extends reads.
    pub fn from_prepared(descriptor: &super::EffectiveHolonDescriptor) -> Result<Self, HolonError> {
        use super::Descriptor;
        let target = descriptor.described_holon_type()?.holon().clone();
        let context = descriptor.holon().resolution_context()?;
        let metadata = descriptor.prepared_schema()?;
        let lineage = metadata
            .lineage
            .iter()
            .map(|id| HolonReference::smart_from_id(context.space_read_handle(), id.clone()))
            .collect::<Vec<_>>();
        let mut restored = Vec::new();
        for (index, (source, result, _, _, _)) in collections().into_iter().enumerate() {
            let members = descriptor
                .holon()
                .related_holons(result.clone())?
                .read()
                .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
                .get_members()
                .clone();
            let origins = metadata.origins.get(index).ok_or_else(|| {
                HolonError::InvalidParameter("Missing prepared declaring origins".into())
            })?;
            if origins.len() != members.len() {
                return Err(HolonError::InvalidParameter(
                    "Prepared product/origin length mismatch".into(),
                ));
            }
            let occurrences = members
                .into_iter()
                .zip(origins)
                .map(|(member, (origin, position))| EffectiveContribution {
                    member,
                    declared_on: HolonReference::smart_from_id(
                        context.space_read_handle(),
                        origin.clone(),
                    ),
                    source_collection: source.clone(),
                    local_position: *position,
                })
                .collect();
            restored.push(EffectiveCollection { source, result, occurrences });
        }
        let navigation = OnceLock::new();
        let navigation_members = descriptor
            .navigable_relationships()?
            .into_iter()
            .zip(&metadata.navigation_origins)
            .map(|(relationship, (origin, position, source_of))| EffectiveContribution {
                member: relationship.holon().clone(),
                declared_on: HolonReference::smart_from_id(
                    context.space_read_handle(),
                    origin.clone(),
                ),
                source_collection: if *source_of {
                    CoreRelationshipTypeName::SourceOf
                } else {
                    CoreRelationshipTypeName::InstanceRelationships
                },
                local_position: *position,
            })
            .collect();
        let _ = navigation.set(Ok(navigation_members));
        Ok(Self {
            target,
            lineage,
            collections: restored,
            navigation,
            properties: OnceLock::new(),
            relationships: OnceLock::new(),
            dances: OnceLock::new(),
            commands: OnceLock::new(),
        })
    }

    /// Resolves a declared member within this already-computed contract.
    pub fn get_relationship_by_name(
        &self,
        name: impl type_names::ToRelationshipName,
    ) -> Result<super::RelationshipDescriptor, HolonError> {
        use super::Descriptor;
        let requested = name.to_relationship_name();
        for member in self.instance_relationships() {
            let descriptor = super::RelationshipDescriptor::from_holon(member.holon().clone());
            if descriptor.base_relationship_name()? == requested {
                return Ok(descriptor);
            }
        }
        Err(HolonError::DescriptorDeclarationNotFound {
            kind: "relationship".into(),
            name: requested.to_string(),
            descriptor: self.target.reference_id_string(),
        })
    }

    /// Typed local views for bootstrap validation, without allocating runtime results.
    pub fn instance_properties(&self) -> Vec<super::PropertyDescriptor> {
        self.contributions(CoreRelationshipTypeName::InstanceProperties)
            .iter()
            .map(|entry| super::PropertyDescriptor::from_holon(entry.member.clone()))
            .collect()
    }
    pub fn instance_relationships(&self) -> Vec<super::DeclaredRelationshipDescriptor> {
        self.contributions(CoreRelationshipTypeName::InstanceRelationships)
            .iter()
            .map(|entry| {
                super::DeclaredRelationshipDescriptor::from_projected_holon(entry.member.clone())
            })
            .collect()
    }
    pub fn effective_key_rule(&self) -> Result<super::KeyRuleDescriptor, HolonError> {
        let entries = self.contributions(CoreRelationshipTypeName::InstanceKeyRule);
        if entries.len() != 1 {
            return Err(HolonError::InvalidParameter(format!(
                "Expected one effective key rule, got {}",
                entries.len()
            )));
        }
        let rule = super::KeyRuleDescriptor::from_holon(entries[0].member.clone());
        rule.require_concrete_strategy()?;
        Ok(rule)
    }

    /// Materializes the result Holon; occurrence provenance stays in this computed projection.
    pub fn materialize(
        &self,
        context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
    ) -> Result<super::EffectiveHolonDescriptor, HolonError> {
        super::effective_holon_descriptor::materialize_effective_schema(context, self, |_, _| {})
    }

    /// Resolves L(T), never L(D(T)); supports invalid but structurally traversable schemas.
    pub fn resolve(target: &HolonReference) -> Result<Self, HolonError> {
        Self::resolve_with_checkpoint(target, &mut |_, _| {})
    }

    /// Reuses the structural lineage already prepared by Commit validation.
    pub fn from_valid_lineage(lineage: ValidExtendsLineage<'_>) -> Result<Self, HolonError> {
        Self::from_members(lineage.members().to_vec(), &mut |_, _| {})
    }

    pub(super) fn resolve_with_checkpoint(
        target: &HolonReference,
        checkpoint: &mut impl FnMut(&str, &str),
    ) -> Result<Self, HolonError> {
        let lineage = ancestors(target)?;
        checkpoint("lineage", "");
        Self::from_members(lineage, checkpoint)
    }

    fn from_members(
        lineage: Vec<HolonReference>,
        checkpoint: &mut impl FnMut(&str, &str),
    ) -> Result<Self, HolonError> {
        let target = lineage
            .first()
            .ok_or_else(|| HolonError::InvalidParameter("Empty descriptor lineage".into()))?
            .clone();
        let mut resolved = Vec::new();
        for (source, result, _, _, _) in collections() {
            let selected = relationship_targets_in_lineage(&lineage, source.clone())?;
            let mut positions = HashMap::<DefinitionIdentity, usize>::new();
            let mut occurrences = Vec::with_capacity(selected.len());
            for contribution in selected {
                let origin = DefinitionIdentity::for_optional_context(
                    &contribution.declared_on,
                    contribution.declared_on.transaction_context().as_ref(),
                )?;
                let position = positions.entry(origin).or_default();
                occurrences.push(EffectiveContribution {
                    member: contribution.member,
                    declared_on: contribution.declared_on,
                    source_collection: source.clone(),
                    local_position: *position,
                });
                *position += 1;
            }
            checkpoint("collection_read", &result.as_relationship_name().to_string());
            resolved.push(EffectiveCollection { source, result, occurrences });
        }
        Ok(Self {
            target,
            lineage,
            collections: resolved,
            navigation: OnceLock::new(),
            properties: OnceLock::new(),
            relationships: OnceLock::new(),
            dances: OnceLock::new(),
            commands: OnceLock::new(),
        })
    }

    /// Checks only the well-known HolonType ancestor key, without resolving an anchor.
    pub fn defines_holon_instances(&self) -> Result<bool, HolonError> {
        let key = CoreHolonTypeName::HolonType.descriptor_key(&CoreHolonTypeName::TypeDescriptor);
        for ancestor in &self.lineage {
            if matches!(ancestor.property_value(CorePropertyTypeName::Key)?, Some(BaseValue::StringValue(ref value)) if value == &key)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn contributions(&self, source: CoreRelationshipTypeName) -> &[EffectiveContribution] {
        self.collections
            .iter()
            .find(|collection| {
                std::mem::discriminant(&collection.source) == std::mem::discriminant(&source)
            })
            .map(|collection| collection.occurrences.as_slice())
            .unwrap_or_default()
    }

    /// Complete navigable vocabulary, retaining conflicting declarations and their origins.
    pub fn navigable_relationships(&self) -> Result<&[EffectiveContribution], HolonError> {
        self.navigation
            .get_or_init(|| {
                effective_relationships::navigable_occurrences_in_lineage(
                    &self.lineage,
                    self.contributions(CoreRelationshipTypeName::InstanceRelationships).to_vec(),
                )
            })
            .as_ref()
            .map(Vec::as_slice)
            .map_err(Clone::clone)
    }

    /// Adapter for existing Commit consumers; the projection retains original positions.
    pub fn contract_contributions(&self) -> ContractContributions {
        let members = |source| {
            self.contributions(source)
                .iter()
                .map(|entry| EffectiveRelationshipMember {
                    member: entry.member.clone(),
                    declared_on: entry.declared_on.clone(),
                })
                .collect()
        };
        ContractContributions {
            properties: members(CoreRelationshipTypeName::InstanceProperties),
            relationships: members(CoreRelationshipTypeName::InstanceRelationships),
        }
    }

    pub fn name_index(
        &self,
        vocabulary: EffectiveVocabulary,
    ) -> Result<&EffectiveNameIndex, HolonError> {
        let cache = match vocabulary {
            EffectiveVocabulary::Properties => &self.properties,
            EffectiveVocabulary::Relationships => &self.relationships,
            EffectiveVocabulary::Dances => &self.dances,
            EffectiveVocabulary::Commands => &self.commands,
        };
        cache
            .get_or_init(|| {
                let occurrences = match vocabulary {
                    EffectiveVocabulary::Properties => {
                        self.contributions(CoreRelationshipTypeName::InstanceProperties)
                    }
                    EffectiveVocabulary::Relationships => self.navigable_relationships()?,
                    EffectiveVocabulary::Dances => {
                        self.contributions(CoreRelationshipTypeName::AffordsDance)
                    }
                    EffectiveVocabulary::Commands => {
                        self.contributions(CoreRelationshipTypeName::AffordsCommand)
                    }
                };
                let mut index = EffectiveNameIndex::default();
                for occurrence in occurrences {
                    match occurrence.member.property_value(CorePropertyTypeName::TypeName)? {
                        Some(BaseValue::StringValue(name)) if !name.0.is_empty() => {
                            index.entries.entry(name).or_default().push(occurrence.clone())
                        }
                        _ => index.unnamed.push(occurrence.clone()),
                    }
                }
                Ok(index)
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Average O(1) lookup after the vocabulary index is built. Conflicts never pick a winner.
    pub fn named(
        &self,
        vocabulary: EffectiveVocabulary,
        name: &MapString,
    ) -> Result<Option<HolonReference>, HolonError> {
        match self.name_index(vocabulary)?.entries.get(name).map(Vec::as_slice).unwrap_or_default()
        {
            [] => Ok(None),
            [only] => Ok(Some(only.member.clone())),
            _ => Err(HolonError::DuplicateInheritedDeclaration {
                kind: vocabulary.label().into(),
                name: name.0.clone(),
                descriptor: self.target.reference_id_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, build_context_with_read_errors, new_descriptor_holon, new_test_holon,
    };
    use crate::WritableHolon;
    use core_types::{HolonId, LocalId, PropertyMap};

    fn fixture() -> Result<
        (
            std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
            crate::TransientReference,
            crate::TransientReference,
        ),
        HolonError,
    > {
        let context = build_context();
        let root = new_descriptor_holon(&context, "HolonType.TypeDescriptor", "HolonType", "")?;
        let root = context.mutation().stage_new_holon(root)?;
        let mut parent = new_test_holon(&context, "projection-parent")?;
        parent.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        let mut child = new_test_holon(&context, "projection-child")?;
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.clone().into()])?;
        let descriptor = new_descriptor_holon(
            &context,
            "EffectiveHolonDescriptor.HolonType",
            "EffectiveHolonDescriptor",
            "",
        )?;
        context.mutation().stage_new_holon(descriptor)?;
        Ok((context, parent, child))
    }

    #[test]
    fn occurrences_and_local_positions_survive_additive_inheritance_and_name_conflicts(
    ) -> Result<(), HolonError> {
        let (context, mut parent, mut child) = fixture()?;
        let mut member = new_test_holon(&context, "repeated-member")?;
        member.with_property_value(CorePropertyTypeName::TypeName, "Name")?;
        let mut conflicting = new_test_holon(&context, "other-member")?;
        conflicting.with_property_value(CorePropertyTypeName::TypeName, "Name")?;
        let mut unique = new_test_holon(&context, "unique-member")?;
        unique.with_property_value(CorePropertyTypeName::TypeName, "Other")?;
        parent.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![member.clone().into(), member.clone().into()],
        )?;
        child.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![conflicting.into(), unique.clone().into(), member.into()],
        )?;
        let before = context.lookup().transient_count()?;
        let projection = ComputedEffectiveSchema::resolve(&child.into())?;
        assert_eq!(context.lookup().transient_count()?, before);
        let occurrences = projection.contributions(CoreRelationshipTypeName::InstanceProperties);
        assert_eq!(occurrences.len(), 5);
        assert_eq!(
            occurrences.iter().map(|entry| entry.local_position).collect::<Vec<_>>(),
            vec![0, 1, 0, 1, 2]
        );
        assert_eq!(
            projection.name_index(EffectiveVocabulary::Properties)?.entries
                [&MapString("Name".into())]
                .len(),
            4
        );
        assert!(projection
            .named(EffectiveVocabulary::Properties, &MapString("Name".into()))
            .is_err());
        assert!(super::super::same_definition(
            &projection
                .named(EffectiveVocabulary::Properties, &MapString("Other".into()))?
                .unwrap(),
            &unique.into()
        )?);
        assert!(std::ptr::eq(
            projection.name_index(EffectiveVocabulary::Properties)?,
            projection.name_index(EffectiveVocabulary::Properties)?
        ));
        Ok(())
    }

    #[test]
    fn override_keeps_nearest_collection_even_when_that_collection_is_invalid(
    ) -> Result<(), HolonError> {
        let (context, mut parent, mut child) = fixture()?;
        let first = new_test_holon(&context, "parent-rule")?;
        let second = new_test_holon(&context, "invalid-child-rule")?;
        parent.add_related_holons(CoreRelationshipTypeName::InstanceKeyRule, vec![first.into()])?;
        child.add_related_holons(
            CoreRelationshipTypeName::InstanceKeyRule,
            vec![second.clone().into(), second.into()],
        )?;
        let projection = ComputedEffectiveSchema::resolve(&child.clone().into())?;
        let selected = projection.contributions(CoreRelationshipTypeName::InstanceKeyRule);
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|entry| super::super::same_definition(
            &entry.declared_on,
            &child.clone().into()
        )
        .unwrap()));
        Ok(())
    }

    #[test]
    fn navigation_keeps_duplicate_declarations_and_distinct_conflicting_candidates(
    ) -> Result<(), HolonError> {
        let (context, _, mut child) = fixture()?;
        let mut declared = new_test_holon(&context, "projection-declared")?;
        declared.with_property_value(CorePropertyTypeName::TypeName, "Peers")?;
        declared
            .add_related_holons(CoreRelationshipTypeName::SourceType, vec![child.clone().into()])?;
        let mut inverse = new_test_holon(&context, "projection-inverse")?;
        inverse.with_property_value(CorePropertyTypeName::TypeName, "Peers")?;
        inverse
            .add_related_holons(CoreRelationshipTypeName::SourceType, vec![child.clone().into()])?;
        child.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![declared.clone().into(), declared.clone().into()],
        )?;
        child.add_related_holons(
            CoreRelationshipTypeName::SourceOf,
            vec![declared.into(), inverse.into()],
        )?;
        let projection = ComputedEffectiveSchema::resolve(&child.into())?;
        assert_eq!(projection.navigable_relationships()?.len(), 3);
        assert!(projection
            .named(EffectiveVocabulary::Relationships, &MapString("Peers".into()))
            .is_err());
        Ok(())
    }

    #[test]
    fn computation_and_materialization_never_fetch_unread_source_members() -> Result<(), HolonError>
    {
        let (context, _, mut child) = fixture()?;
        let missing: HolonReference = HolonReference::smart_from_id(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![77; 39])),
        );
        child
            .add_related_references(CoreRelationshipTypeName::InstanceProperties, vec![missing])?;
        let before = context.lookup().transient_count()?;
        let projection = ComputedEffectiveSchema::resolve(&child.into())?;
        assert_eq!(context.lookup().transient_count()?, before);
        let result = projection.materialize(&context)?;
        assert_eq!(context.lookup().transient_count()?, before + 1);
        assert_eq!(result.instance_properties()?.len(), 1);
        assert!(result.contributions()?.is_empty());
        Ok(())
    }

    #[test]
    fn membership_reads_do_not_load_source_content_but_actual_content_errors_survive(
    ) -> Result<(), HolonError> {
        let id = HolonId::Local(LocalId(vec![42; 39]));
        let saved = crate::core_shared_objects::holon::SavedHolon::new(
            id.local_id().clone(),
            PropertyMap::new(),
            None,
            base_types::MapInteger(1),
        );
        let context = build_context_with_read_errors(
            vec![saved],
            HashMap::new(),
            HashMap::from([(
                id.clone(),
                HolonError::InvalidState("content read forbidden".into()),
            )]),
        );
        let reference: HolonReference = crate::SmartReference::new_with_properties(
            context.space_read_handle(),
            id,
            PropertyMap::from([(
                type_names::ToPropertyName::to_property_name(CorePropertyTypeName::Key),
                BaseValue::StringValue(MapString("saved-source".into())),
            )]),
        )
        .into();
        assert!(reference
            .related_holons(CoreRelationshipTypeName::Extends)?
            .read()
            .unwrap()
            .get_members()
            .is_empty());
        assert!(reference.property_value(CorePropertyTypeName::TypeName).is_err());
        Ok(())
    }
}
