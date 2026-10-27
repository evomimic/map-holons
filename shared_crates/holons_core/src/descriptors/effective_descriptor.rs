//! Transaction-scoped, reference-backed resolved schema and contribution provenance.
use std::sync::Arc;

use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::{
    CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName, ToRelationshipName,
};

use super::{accessor_helpers, *};
use crate::core_shared_objects::holon::state::AccessType;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::{DefinitionIdentity, HolonReference, ReadableHolon, WritableHolon};

/// One immutable derived schema graph retained by the caller's transaction.
/// Collections are read from relationships; the wrapper retains no resolution cache.
pub struct EffectiveDescriptor {
    holon: HolonReference,
}

impl EffectiveDescriptor {
    /// Wraps the successful result of the typed construction service without revalidation.
    /// Arbitrary references must use `try_from_holon` instead.
    pub fn from_constructed_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Narrows a readable transient result by its describing type.
    pub fn try_from_holon(holon: HolonReference) -> Result<Self, HolonError> {
        require_transient_type(&holon, CoreHolonTypeName::EffectiveDescriptor)?;
        Ok(Self { holon })
    }

    /// Returns the retained result reference, suitable for ordinary transport.
    pub fn holon(&self) -> &HolonReference {
        &self.holon
    }

    /// Returns the authoritative type whose schema was resolved.
    pub fn described_holon_type(&self) -> Result<HolonDescriptor, HolonError> {
        // Construction established target eligibility using its retained lineage.
        Ok(HolonDescriptor::from_holon(single(
            &self.holon,
            CoreRelationshipTypeName::DescribedHolonType,
        )?))
    }

    /// Reads selected property occurrences without revalidating their definitions.
    pub fn instance_properties(&self) -> Result<Vec<PropertyDescriptor>, HolonError> {
        Ok(targets(&self.holon, CoreRelationshipTypeName::EffectiveInstanceProperties)?
            .into_iter()
            .map(PropertyDescriptor::from_holon)
            .collect())
    }

    /// Reads projected declaration roles without reclassifying the source schema.
    pub fn instance_relationships(
        &self,
    ) -> Result<Vec<DeclaredRelationshipDescriptor>, HolonError> {
        Ok(targets(&self.holon, CoreRelationshipTypeName::EffectiveInstanceRelationships)?
            .into_iter()
            .map(DeclaredRelationshipDescriptor::from_projected_holon)
            .collect())
    }

    /// Reads outbound declared and inverse schema vocabulary, independently of instances.
    pub fn navigable_relationships(&self) -> Result<Vec<QualifiedRelationship>, HolonError> {
        targets(&self.holon, CoreRelationshipTypeName::NavigableRelationships)?
            .into_iter()
            .map(|target| {
                let descriptor_direction = classify_relationship_direction(&target)?;
                Ok(QualifiedRelationship {
                    descriptor: RelationshipDescriptor::from_holon(target),
                    descriptor_direction,
                })
            })
            .collect()
    }

    /// Reads resolved Dance affordances from the completed graph.
    pub fn afforded_dances(&self) -> Result<Vec<DanceDescriptor>, HolonError> {
        Ok(targets(&self.holon, CoreRelationshipTypeName::EffectiveAffordsDance)?
            .into_iter()
            .map(DanceDescriptor::from_holon)
            .collect())
    }

    pub fn afforded_commands(&self) -> Result<Vec<CommandDescriptor>, HolonError> {
        Ok(targets(&self.holon, CoreRelationshipTypeName::EffectiveAffordsCommand)?
            .into_iter()
            .map(CommandDescriptor::from_holon)
            .collect())
    }

    pub fn effective_constraints(&self) -> Result<Vec<HolonReference>, HolonError> {
        targets(&self.holon, CoreRelationshipTypeName::EffectiveConstraints)
    }

    pub fn effective_validation_bindings(&self) -> Result<Vec<HolonReference>, HolonError> {
        targets(&self.holon, CoreRelationshipTypeName::EffectiveValidationBindings)
    }

    pub fn effective_key_rule(&self) -> Result<KeyRuleDescriptor, HolonError> {
        Ok(KeyRuleDescriptor::from_holon(single(
            &self.holon,
            CoreRelationshipTypeName::EffectiveInstanceKeyRule,
        )?))
    }

    /// Explicitly validates contribution sequences against their projected result products.
    /// Construction and ordinary projection reads never call this method.
    /// Uses only retained references and source occurrences, never the target lineage.
    pub fn validate_contributions(&self) -> Result<(), HolonError> {
        let context = self.holon.resolution_context()?;
        let mut grouped =
            std::collections::HashMap::<String, Vec<(i64, DefinitionIdentity)>>::new();
        for record in self.contributions()? {
            record.validate_provenance()?;
            let name = record.collection()?.base_relationship_name()?.to_string();
            grouped.entry(name).or_default().push((
                record.position()?,
                DefinitionIdentity::for_reference(&record.member()?.member, &context)?,
            ));
        }
        for (source, result, _, _, _) in collections() {
            let mut records =
                grouped.remove(&source.to_relationship_name().to_string()).unwrap_or_default();
            records.sort_by_key(|(position, _)| *position);
            for (index, (position, _)) in records.iter().enumerate() {
                if *position != index as i64 {
                    return Err(HolonError::InvalidParameter(
                        "Contribution positions must be contiguous and unique".into(),
                    ));
                }
            }
            let direct = targets(&self.holon, result)?
                .into_iter()
                .map(|member| DefinitionIdentity::for_reference(&member, &context))
                .collect::<Result<Vec<_>, _>>()?;
            if records.is_empty() && direct.is_empty() {
                continue;
            }
            let expected = records.into_iter().map(|(_, member)| member).collect::<Vec<_>>();
            let mut actual_counts = std::collections::HashMap::new();
            let mut expected_counts = std::collections::HashMap::new();
            for member in &direct {
                *actual_counts.entry(member.clone()).or_insert(0_usize) += 1;
            }
            for member in &expected {
                *expected_counts.entry(member.clone()).or_insert(0_usize) += 1;
            }
            if actual_counts != expected_counts || direct != expected {
                return Err(HolonError::InvalidParameter(
                    "Contribution sequence does not match the effective product".into(),
                ));
            }
        }
        Ok(())
    }

    /// Reads distinct contribution records. Position is scoped to each source collection.
    pub fn contributions(&self) -> Result<Vec<EffectiveSchemaContribution>, HolonError> {
        targets(&self.holon, CoreRelationshipTypeName::EffectiveContributions)?
            .into_iter()
            .map(|holon| Ok(EffectiveSchemaContribution { holon }))
            .collect()
    }
}

/// One selected source occurrence, represented entirely by ordinary graph state.
pub struct EffectiveSchemaContribution {
    holon: HolonReference,
}
impl EffectiveSchemaContribution {
    /// Narrows a readable transient provenance record.
    pub fn try_from_holon(holon: HolonReference) -> Result<Self, HolonError> {
        require_transient_type(&holon, CoreHolonTypeName::EffectiveSchemaContribution)?;
        Ok(Self { holon })
    }
    /// Returns this record's ordinary transient reference.
    pub fn holon(&self) -> &HolonReference {
        &self.holon
    }
    /// Identifies the authoritative source collection, rather than the result relationship.
    pub fn collection(&self) -> Result<RelationshipDescriptor, HolonError> {
        Ok(RelationshipDescriptor::from_holon(single(
            &self.holon,
            CoreRelationshipTypeName::ContributionCollection,
        )?))
    }

    fn checked_collection(&self) -> Result<RelationshipDescriptor, HolonError> {
        let reference = single(&self.holon, CoreRelationshipTypeName::ContributionCollection)?;
        require_descriptor_category(
            &reference,
            &CoreHolonTypeName::RelationshipType
                .descriptor_key(&CoreHolonTypeName::TypeDescriptor)
                .0,
        )?;
        // Collection authority is independent of relationship direction. Only the
        // seven canonical collections used by construction may identify origins.
        let context = reference.resolution_context()?;
        for (_, _, key, _, _) in collections() {
            match resolve_core_descriptor(&context, &key.0) {
                Ok(expected) if same_definition(&reference, &expected)? => {
                    return Ok(RelationshipDescriptor::from_holon(reference));
                }
                Ok(_) | Err(HolonError::HolonNotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        Err(HolonError::InvalidParameter("Not an authoritative contribution collection".into()))
    }
    /// Projects the existing provenance shape from this record's relationships.
    pub fn member(&self) -> Result<EffectiveRelationshipMember, HolonError> {
        Ok(EffectiveRelationshipMember {
            member: single(&self.holon, CoreRelationshipTypeName::ContributionMember)?,
            declared_on: single(&self.holon, CoreRelationshipTypeName::ContributionDeclaredOn)?,
        })
    }
    /// Checks the record against its actual authoritative source occurrence.
    /// Does not resolve the target type's lineage or normalize contributions.
    pub fn validate_provenance(&self) -> Result<(), HolonError> {
        let collection = self.checked_collection()?;
        let contribution = self.member()?;
        self.position()?;
        let name = collection.base_relationship_name()?;
        let (_, _, _, category, contract) = collections()
            .into_iter()
            .find(|(source, ..)| source.to_relationship_name() == name)
            .ok_or_else(|| {
                HolonError::InvalidParameter("Unknown contribution collection".into())
            })?;
        if contract {
            require_descriptor_category(&contribution.member, &category.0)?;
        } else if name == CoreRelationshipTypeName::InstanceKeyRule.as_relationship_name() {
            KeyRuleDescriptor::from_holon(contribution.member.clone())
                .require_concrete_strategy()?;
        } else {
            let context = self.holon.resolution_context()?;
            if !has_type(&contribution.member, &resolve_core_descriptor(&context, &category.0)?)? {
                return Err(HolonError::InvalidParameter(
                    "Contribution member has the wrong configured type".into(),
                ));
            }
        }
        let members = targets(&contribution.declared_on, name)?;
        for member in members {
            if same_definition(&member, &contribution.member)? {
                return Ok(());
            }
        }
        Err(HolonError::InvalidParameter(
            "Contribution member is absent from its declaring collection".into(),
        ))
    }

    /// Returns this occurrence's nonnegative position in its source collection's sequence.
    pub fn position(&self) -> Result<i64, HolonError> {
        match self.holon.property_value(CorePropertyTypeName::ContributionPosition)? {
            Some(BaseValue::IntegerValue(position)) if position.0 >= 0 => Ok(position.0),
            other => Err(HolonError::InvalidParameter(format!(
                "Invalid ContributionPosition: {other:?}"
            ))),
        }
    }
}

/// Constructs one coherent resolved schema using exactly one complete target-lineage walk.
/// The caller must keep the contributing schema graph unchanged during construction and
/// reconstruct after relevant edits. No instances or replacement definitions are consulted.
pub fn construct_effective_descriptor(
    context: &Arc<TransactionContext>,
    target: HolonReference,
) -> Result<EffectiveDescriptor, HolonError> {
    construct_effective_descriptor_impl(context, target, false, |_, _| {})
}

/// Explicitly materializes contribution Holons for graph-oriented provenance consumers.
pub fn construct_effective_descriptor_with_contributions(
    context: &Arc<TransactionContext>,
    target: HolonReference,
) -> Result<EffectiveDescriptor, HolonError> {
    construct_effective_descriptor_impl(context, target, true, |_, _| {})
}

/// Controlled profiling entry point; production always retains contribution provenance.
#[cfg(feature = "descriptor-performance-profiling")]
pub fn profile_effective_descriptor_construction(
    context: &Arc<TransactionContext>,
    target: HolonReference,
    include_contributions: bool,
    checkpoint: impl FnMut(&str, &str),
) -> Result<EffectiveDescriptor, HolonError> {
    construct_effective_descriptor_impl(context, target, include_contributions, checkpoint)
}

fn construct_effective_descriptor_impl(
    context: &Arc<TransactionContext>,
    target: HolonReference,
    include_contributions: bool,
    mut checkpoint: impl FnMut(&str, &str),
) -> Result<EffectiveDescriptor, HolonError> {
    let computed = ComputedEffectiveSchema::resolve_with_checkpoint(&target, &mut checkpoint)?;
    materialize_effective_schema(context, &computed, include_contributions, checkpoint)
}

/// Materializes an already computed projection without traversing its lineage again.
pub(super) fn materialize_effective_schema(
    context: &Arc<TransactionContext>,
    computed: &ComputedEffectiveSchema,
    include_contributions: bool,
    mut checkpoint: impl FnMut(&str, &str),
) -> Result<EffectiveDescriptor, HolonError> {
    let target = computed.target.clone();
    if !computed.defines_holon_instances()? {
        return Err(HolonError::WrongDescriptorKind {
            expected: CoreHolonTypeName::HolonType.as_holon_name().0,
            found: "other descriptor".into(),
            descriptor: target.reference_id_string(),
        });
    }
    checkpoint("target_category", "");
    let navigable =
        computed.navigable_relationships()?.iter().map(|entry| entry.member.clone()).collect();
    checkpoint("navigation", "");
    let result_type = resolve_core_descriptor(
        context,
        &CoreHolonTypeName::EffectiveDescriptor.descriptor_key(&CoreHolonTypeName::HolonType).0,
    )?;
    let contribution_type = if include_contributions {
        Some(resolve_core_descriptor(
            context,
            &CoreHolonTypeName::EffectiveSchemaContribution
                .descriptor_key(&CoreHolonTypeName::HolonType)
                .0,
        )?)
    } else {
        None
    };
    let owner = context.get_space_holon()?;
    let mut result = context.mutation().new_holon(None)?;
    result.add_related_references(CoreRelationshipTypeName::DescribedBy, vec![result_type])?;
    if let Some(space) = &owner {
        result.add_related_references(CoreRelationshipTypeName::OwnedBy, vec![space.clone()])?;
    }
    result.add_related_references(CoreRelationshipTypeName::DescribedHolonType, vec![target])?;
    result.add_related_references(CoreRelationshipTypeName::NavigableRelationships, navigable)?;
    checkpoint("result_allocation", "");
    let mut records = Vec::new();
    for collection in &computed.collections {
        let result_name = collection.result.clone();
        let source_key = &collection.source_key;
        let contributions = &collection.occurrences;
        let values = contributions.iter().map(|entry| entry.member.clone()).collect();
        let result_label = result_name.as_relationship_name().to_string();
        result.add_related_references(result_name, values)?;
        checkpoint("product_population", &result_label);
        if let Some(contribution_type) =
            contribution_type.as_ref().filter(|_| !contributions.is_empty())
        {
            let source = resolve_core_descriptor(context, &source_key.0)?;
            for (position, contribution) in contributions.iter().enumerate() {
                let mut record = context.mutation().new_holon(None)?;
                record.add_related_references(
                    CoreRelationshipTypeName::DescribedBy,
                    vec![contribution_type.clone()],
                )?;
                if let Some(space) = &owner {
                    record.add_related_references(
                        CoreRelationshipTypeName::OwnedBy,
                        vec![space.clone()],
                    )?;
                }
                record.with_property_value(
                    CorePropertyTypeName::ContributionPosition,
                    position as i64,
                )?;
                record.add_related_references(
                    CoreRelationshipTypeName::ContributionCollection,
                    vec![source.clone()],
                )?;
                record.add_related_references(
                    CoreRelationshipTypeName::ContributionMember,
                    vec![contribution.member.clone()],
                )?;
                record.add_related_references(
                    CoreRelationshipTypeName::ContributionDeclaredOn,
                    vec![contribution.declared_on.clone()],
                )?;
                record.mark_as_immutable()?;
                records.push(record.into());
            }
        }
        checkpoint("contribution_allocation", &result_label);
    }
    result.add_related_references(CoreRelationshipTypeName::EffectiveContributions, records)?;
    result.mark_as_immutable()?;
    let result = Ok(EffectiveDescriptor { holon: result.into() });
    checkpoint("finalize", "");
    result
}

// Descriptor Runtime owns result pairs; authoritative collections retain their package identities.
pub(super) fn collections(
) -> [(CoreRelationshipTypeName, CoreRelationshipTypeName, MapString, MapString, bool); 7] {
    let holon_type =
        CoreHolonTypeName::HolonType.descriptor_key(&CoreHolonTypeName::TypeDescriptor);
    let type_descriptor = CoreHolonTypeName::TypeDescriptor.as_holon_name();
    [
        (
            CoreRelationshipTypeName::InstanceProperties,
            CoreRelationshipTypeName::EffectiveInstanceProperties,
            holon_type.clone(),
            CoreHolonTypeName::PropertyType.descriptor_key(&CoreHolonTypeName::TypeDescriptor),
            true,
        ),
        (
            CoreRelationshipTypeName::InstanceRelationships,
            CoreRelationshipTypeName::EffectiveInstanceRelationships,
            holon_type.clone(),
            CoreHolonTypeName::DeclaredRelationshipType
                .descriptor_key(&CoreHolonTypeName::RelationshipType),
            true,
        ),
        (
            CoreRelationshipTypeName::AffordsDance,
            CoreRelationshipTypeName::EffectiveAffordsDance,
            holon_type.clone(),
            CoreHolonTypeName::DanceType.descriptor_key(&CoreHolonTypeName::HolonType),
            true,
        ),
        (
            CoreRelationshipTypeName::AffordsCommand,
            CoreRelationshipTypeName::EffectiveAffordsCommand,
            holon_type.clone(),
            CoreHolonTypeName::CommandType.descriptor_key(&CoreHolonTypeName::HolonType),
            true,
        ),
        (
            CoreRelationshipTypeName::ValidationBindings,
            CoreRelationshipTypeName::EffectiveValidationBindings,
            type_descriptor.clone(),
            CoreHolonTypeName::ValidationRule.descriptor_key(&CoreHolonTypeName::HolonType),
            false,
        ),
        (
            CoreRelationshipTypeName::Constraints,
            CoreRelationshipTypeName::EffectiveConstraints,
            type_descriptor,
            CoreHolonTypeName::Constraint.descriptor_key(&CoreHolonTypeName::HolonType),
            false,
        ),
        (
            CoreRelationshipTypeName::InstanceKeyRule,
            CoreRelationshipTypeName::EffectiveInstanceKeyRule,
            holon_type,
            CoreHolonTypeName::KeyRuleType.descriptor_key(&CoreHolonTypeName::HolonType),
            false,
        ),
    ]
    .map(|(source, result, source_type, category, contract)| {
        let key = source.descriptor_key(&source_type, &category);
        (source, result, key, category, contract)
    })
}

fn require_descriptor_category(target: &HolonReference, key: &str) -> Result<(), HolonError> {
    let anchor = resolve_core_descriptor(&target.resolution_context()?, key)?;
    if equals_or_extends(target, &anchor)? {
        return Ok(());
    }
    Err(HolonError::WrongDescriptorKind {
        expected: key.into(),
        found: "other descriptor".into(),
        descriptor: target.reference_id_string(),
    })
}
fn require_transient_type(
    holon: &HolonReference,
    name: CoreHolonTypeName,
) -> Result<(), HolonError> {
    let name = name.as_holon_name();
    if !holon.is_transient() {
        return Err(HolonError::InvalidHolonReference(format!("{name} must be transient")));
    }
    holon.is_accessible(AccessType::Read)?;
    require_holon_type(holon, &name)
}
fn targets(
    holon: &HolonReference,
    name: impl ToRelationshipName,
) -> Result<Vec<HolonReference>, HolonError> {
    let collection = holon.related_holons(name)?;
    let members = collection.read().map_err(accessor_helpers::lock_error)?.get_members().clone();
    Ok(members)
}
fn single(
    holon: &HolonReference,
    name: CoreRelationshipTypeName,
) -> Result<HolonReference, HolonError> {
    accessor_helpers::require_single_related(holon, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_key_rule_context, new_descriptor_holon, new_test_holon,
    };
    use crate::HolonCollectionApi;

    fn canonical(
        context: &Arc<TransactionContext>,
        key: &str,
        name: &str,
    ) -> Result<HolonReference, HolonError> {
        Ok(context
            .mutation()
            .stage_new_holon(new_descriptor_holon(context, key, name, "")?)?
            .into())
    }

    fn fixture() -> Result<(Arc<TransactionContext>, HolonReference, HolonReference), HolonError> {
        let context = build_key_rule_context()?;
        let descriptor_root = canonical(&context, "TypeDescriptor", "TypeDescriptor")?;
        let mut root = new_descriptor_holon(&context, "HolonType.TypeDescriptor", "HolonType", "")?;
        root.add_related_holons(CoreRelationshipTypeName::Extends, vec![descriptor_root])?;
        let root: HolonReference = context.mutation().stage_new_holon(root)?.into();
        canonical(
            &context,
            &CoreHolonTypeName::EffectiveDescriptor.descriptor_key(&CoreHolonTypeName::HolonType).0,
            "EffectiveDescriptor",
        )?;
        canonical(
            &context,
            &CoreHolonTypeName::EffectiveSchemaContribution
                .descriptor_key(&CoreHolonTypeName::HolonType)
                .0,
            "EffectiveSchemaContribution",
        )?;
        canonical(&context, "Constraint.HolonType", "Constraint")?;
        canonical(&context, "ValidationRule.HolonType", "ValidationRule")?;
        let relationship_root =
            canonical(&context, "RelationshipType.TypeDescriptor", "RelationshipType")?;
        for (key, name) in [
            ("DeclaredRelationshipType.RelationshipType", "DeclaredRelationshipType"),
            ("InverseRelationshipType.RelationshipType", "InverseRelationshipType"),
        ] {
            let mut category = new_descriptor_holon(&context, key, name, "")?;
            category.add_related_holons(
                CoreRelationshipTypeName::Extends,
                vec![relationship_root.clone()],
            )?;
            context.mutation().stage_new_holon(category)?;
        }
        for (name, _, key, _, _) in collections() {
            if matches!(
                name,
                CoreRelationshipTypeName::AffordsDance | CoreRelationshipTypeName::AffordsCommand
            ) {
                continue;
            }
            let mut source = new_descriptor_holon(&context, &key.0, &format!("{name:?}"), "")?;
            source.with_property_value(
                "AllowsDuplicates",
                matches!(name, CoreRelationshipTypeName::Constraints),
            )?;
            source.with_property_value("IsOrdered", false)?;
            source.add_related_holons(
                CoreRelationshipTypeName::Extends,
                vec![resolve_core_descriptor(
                    &context,
                    "DeclaredRelationshipType.RelationshipType",
                )?],
            )?;
            context.mutation().stage_new_holon(source)?;
        }
        let mut parent = new_test_holon(&context, "parent")?;
        parent.add_related_holons(CoreRelationshipTypeName::Extends, vec![root])?;
        parent.add_related_holons(
            CoreRelationshipTypeName::InstanceKeyRule,
            vec![resolve_core_descriptor(&context, "NoneRule.KeyRuleType")?],
        )?;
        let parent: HolonReference = context.mutation().stage_new_holon(parent)?.into();
        let mut child = new_test_holon(&context, "child")?;
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.clone()])?;
        Ok((context, parent, child.into()))
    }

    #[test]
    fn unsaved_schema_result_is_keyless_sealed_and_walks_target_lineage_once(
    ) -> Result<(), HolonError> {
        let (context, parent, target) = fixture()?;
        let target_before = inheritance::lineage_edge_reads(&target);
        let parent_before = inheritance::lineage_edge_reads(&parent);
        let result = construct_effective_descriptor_with_contributions(&context, target.clone())?;
        assert!(result.holon().key()?.is_none());
        assert!(same_definition(result.described_holon_type()?.holon(), &target)?);
        assert!(result.instance_properties()?.is_empty());
        assert!(result.instance_relationships()?.is_empty());
        assert!(result.navigable_relationships()?.is_empty());
        assert!(result.afforded_dances()?.is_empty());
        assert!(result.afforded_commands()?.is_empty());
        assert!(result.effective_constraints()?.is_empty());
        assert!(result.effective_validation_bindings()?.is_empty());
        assert!(result.effective_key_rule()?.is_keyless()?);
        let contributions = result.contributions()?;
        assert_eq!(contributions.len(), 1);
        assert_eq!(contributions[0].position()?, 0);
        assert!(same_definition(&contributions[0].member()?.declared_on, &parent)?);
        assert_eq!(inheritance::lineage_edge_reads(&target) - target_before, 1);
        assert_eq!(inheritance::lineage_edge_reads(&parent) - parent_before, 1);
        let mut reference = result.holon().clone();
        assert!(reference.with_property_value("Corrupt", true).is_err());
        let collection = reference.related_holons("EffectiveContributions")?;
        assert!(collection.write().unwrap().add_references(vec![target.clone()]).is_err());
        let second = construct_effective_descriptor_with_contributions(&context, target)?;
        assert!(!same_definition(result.holon(), second.holon())?);
        context.dispose()?;
        assert!(result.effective_key_rule().is_err());
        Ok(())
    }

    #[test]
    fn repeated_contributions_retain_distinct_records_and_positions() -> Result<(), HolonError> {
        let (context, mut parent, mut child) = fixture()?;
        let constraint_type = resolve_core_descriptor(&context, "Constraint.HolonType")?;
        let mut constraint = new_test_holon(&context, "constraint")?;
        constraint.with_descriptor(constraint_type)?;
        let constraint: HolonReference = context.mutation().stage_new_holon(constraint)?.into();
        parent
            .add_related_holons(CoreRelationshipTypeName::Constraints, vec![constraint.clone()])?;
        child.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![constraint.clone(), constraint.clone()],
        )?;
        let direct = HolonDescriptor::from_holon(child.clone()).effective_constraints()?;
        let result = construct_effective_descriptor_with_contributions(&context, child)?;
        assert_eq!(result.effective_constraints()?.len(), direct.len());
        let mut records = result
            .contributions()?
            .into_iter()
            .filter(|record| {
                record.collection().unwrap().base_relationship_name().unwrap().to_string()
                    == "Constraints"
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.position().unwrap());
        assert_eq!(records.len(), 3);
        for (position, record) in records.iter().enumerate() {
            assert_eq!(record.position()?, position as i64);
            assert!(same_definition(&record.member()?.member, &constraint)?);
            assert!(same_definition(&record.member()?.declared_on, &direct[position].declared_on)?);
        }
        assert!(!same_definition(records[1].holon(), records[2].holon())?);
        Ok(())
    }

    #[test]
    fn populated_collections_match_direct_products_and_key_override_origin(
    ) -> Result<(), HolonError> {
        let (context, mut parent, mut child) = fixture()?;
        for (source_name, _, key, category, contract) in collections() {
            if matches!(source_name, CoreRelationshipTypeName::InstanceKeyRule) {
                continue;
            }
            let anchor = match resolve_core_descriptor(&context, &category.0) {
                Ok(anchor) => anchor,
                Err(HolonError::HolonNotFound(_)) => {
                    canonical(&context, &category.0, category.0.split('.').next().unwrap())?
                }
                Err(error) => return Err(error),
            };
            if matches!(
                source_name,
                CoreRelationshipTypeName::AffordsDance | CoreRelationshipTypeName::AffordsCommand
            ) {
                let mut source =
                    new_descriptor_holon(&context, &key.0, &format!("{source_name:?}"), "")?;
                source.with_property_value("AllowsDuplicates", false)?;
                source.with_property_value("IsOrdered", false)?;
                source.add_related_holons(
                    CoreRelationshipTypeName::Extends,
                    vec![resolve_core_descriptor(
                        &context,
                        "InverseRelationshipType.RelationshipType",
                    )?],
                )?;
                context.mutation().stage_new_holon(source)?;
            }
            let mut member = new_descriptor_holon(
                &context,
                &format!("{source_name:?}-member"),
                &format!("{source_name:?}Member"),
                "",
            )?;
            if contract {
                member.add_related_holons(CoreRelationshipTypeName::Extends, vec![anchor])?;
                if matches!(source_name, CoreRelationshipTypeName::InstanceRelationships) {
                    member.add_related_holons(
                        CoreRelationshipTypeName::SourceType,
                        vec![parent.clone()],
                    )?;
                    member.add_related_holons(
                        CoreRelationshipTypeName::TargetType,
                        vec![parent.clone()],
                    )?;
                }
            } else {
                member.with_descriptor(anchor)?;
            }
            let member: HolonReference = context.mutation().stage_new_holon(member)?.into();
            parent.add_related_holons(source_name.clone(), vec![member.clone()])?;
        }
        child.add_related_holons(
            CoreRelationshipTypeName::InstanceKeyRule,
            vec![resolve_core_descriptor(&context, "NoneRule.KeyRuleType")?],
        )?;
        let direct = HolonDescriptor::from_holon(child.clone());
        let result = construct_effective_descriptor_with_contributions(&context, child.clone())?;
        result.validate_contributions()?;
        let ids = |references: Vec<HolonReference>| {
            references.into_iter().map(|r| r.full_reference_id_string()).collect::<Vec<_>>()
        };
        assert_eq!(
            ids(result.instance_properties()?.into_iter().map(|p| p.holon().clone()).collect()),
            ids(direct.instance_properties()?.into_iter().map(|p| p.holon().clone()).collect())
        );
        assert_eq!(
            ids(result.instance_relationships()?.into_iter().map(|r| r.holon().clone()).collect()),
            ids(direct.instance_relationships()?.into_iter().map(|r| r.holon().clone()).collect())
        );
        assert_eq!(
            ids(result.afforded_dances()?.into_iter().map(|d| d.holon().clone()).collect()),
            ids(direct.afforded_dances()?.into_iter().map(|d| d.holon().clone()).collect())
        );
        assert_eq!(
            ids(result.afforded_commands()?.into_iter().map(|c| c.holon().clone()).collect()),
            ids(direct.afforded_commands()?.into_iter().map(|c| c.holon().clone()).collect())
        );
        assert_eq!(
            ids(result.effective_constraints()?),
            ids(direct.effective_constraints()?.into_iter().map(|c| c.member).collect())
        );
        assert_eq!(
            ids(result.effective_validation_bindings()?),
            ids(direct.effective_validation_bindings()?.into_iter().map(|b| b.member).collect())
        );
        assert!(same_definition(
            result.effective_key_rule()?.holon(),
            direct.effective_key_rule()?.holon()
        )?);
        assert_eq!(result.navigable_relationships()?.len(), 1);
        assert_eq!(result.contributions()?.len(), 7);
        let key_rule = result.effective_key_rule()?.holon().clone();
        for record in result.contributions()? {
            let contribution = record.member()?;
            let collection = record.collection()?;
            let source_name = collection.base_relationship_name()?.to_string();
            let (_, _, key, _, _) = collections()
                .into_iter()
                .find(|(name, ..)| format!("{name:?}") == source_name)
                .unwrap();
            assert!(same_definition(
                collection.holon(),
                &resolve_core_descriptor(&context, &key.0)?
            )?);
            if source_name == "AffordsDance" || source_name == "AffordsCommand" {
                assert!(matches!(
                    classify_relationship_direction(collection.holon())?,
                    RelationshipDirection::Inverse
                ));
            }
            assert_ne!(source_name, "NavigableRelationships");
            assert!(same_definition(
                &contribution.declared_on,
                if same_definition(&contribution.member, &key_rule)? { &child } else { &parent }
            )?);
            assert_eq!(record.position()?, 0);
        }
        Ok(())
    }

    #[test]
    fn arbitrary_relationship_cannot_identify_a_contribution_collection() -> Result<(), HolonError>
    {
        let (context, _, _) = fixture()?;
        let mut unrelated =
            new_descriptor_holon(&context, "Unrelated.Relationship", "InstanceProperties", "")?;
        unrelated.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![resolve_core_descriptor(&context, "RelationshipType.TypeDescriptor")?],
        )?;
        let mut record = context.mutation().new_holon(None)?;
        record.with_descriptor(resolve_core_descriptor(
            &context,
            &CoreHolonTypeName::EffectiveSchemaContribution
                .descriptor_key(&CoreHolonTypeName::HolonType)
                .0,
        )?)?;
        record.add_related_holons("ContributionCollection", vec![unrelated.into()])?;
        let record = EffectiveSchemaContribution::try_from_holon(record.into())?;
        assert!(matches!(record.validate_provenance(), Err(HolonError::InvalidParameter(_))));
        Ok(())
    }

    #[test]
    fn contribution_origin_must_actually_supply_the_member() -> Result<(), HolonError> {
        let (context, _, target) = fixture()?;
        let result = construct_effective_descriptor_with_contributions(&context, target)?;
        let original = result.contributions()?.remove(0);
        let mut record = context.mutation().new_holon(None)?;
        record.with_descriptor(resolve_core_descriptor(
            &context,
            &CoreHolonTypeName::EffectiveSchemaContribution
                .descriptor_key(&CoreHolonTypeName::HolonType)
                .0,
        )?)?;
        record.with_property_value("ContributionPosition", 0_i64)?;
        record.add_related_holons(
            "ContributionCollection",
            vec![original.collection()?.holon().clone()],
        )?;
        record.add_related_holons("ContributionMember", vec![original.member()?.member])?;
        let unrelated = new_test_holon(&context, "unrelated-origin")?;
        record.add_related_holons("ContributionDeclaredOn", vec![unrelated.into()])?;
        let record = EffectiveSchemaContribution::try_from_holon(record.into())?;
        assert!(matches!(record.validate_provenance(), Err(HolonError::InvalidParameter(_))));
        Ok(())
    }

    #[test]
    fn staged_child_with_no_materialized_inverse_index_constructs() -> Result<(), HolonError> {
        let (context, _, child) = fixture()?;
        let HolonReference::Transient(child) = child else { unreachable!() };
        let child = context.mutation().stage_new_holon(child)?;
        let result = construct_effective_descriptor_with_contributions(&context, child.into())?;
        assert!(result.navigable_relationships()?.is_empty());
        Ok(())
    }

    #[test]
    fn cycles_fail_but_invalid_key_rule_cardinality_remains_projectable() -> Result<(), HolonError>
    {
        let (context, _, mut child) = fixture()?;
        let mut cycle = new_test_holon(&context, "cycle")?;
        cycle.add_related_holons(CoreRelationshipTypeName::Extends, vec![cycle.clone().into()])?;
        assert!(construct_effective_descriptor_with_contributions(&context, cycle.into()).is_err());
        child.add_related_holons(
            CoreRelationshipTypeName::InstanceKeyRule,
            vec![resolve_core_descriptor(&context, "NoneRule.KeyRuleType")?; 2],
        )?;
        let result = construct_effective_descriptor_with_contributions(&context, child)?;
        assert_eq!(
            targets(result.holon(), CoreRelationshipTypeName::EffectiveInstanceKeyRule)?.len(),
            2
        );
        let root = resolve_core_descriptor(&context, "HolonType.TypeDescriptor")?;
        let result = construct_effective_descriptor_with_contributions(&context, root)?;
        assert!(
            targets(result.holon(), CoreRelationshipTypeName::EffectiveInstanceKeyRule)?.is_empty()
        );
        Ok(())
    }
}
