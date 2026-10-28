//! Reference-backed effective Holon schema, prepared eagerly and owned by its runtime context.
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

/// One immutable derived Holon schema graph retained by its owning Session or candidate context.
/// Collections are read from relationships; the wrapper retains no resolution cache.
pub struct EffectiveHolonDescriptor {
    holon: HolonReference,
}

impl EffectiveHolonDescriptor {
    /// Acquires the schema of this exact HolonType, rather than its describing meta-type.
    pub fn acquire(target: HolonReference) -> Result<Self, HolonError> {
        let context = target.resolution_context()?;
        context.get_holon_service().acquire_effective_holon_descriptor_internal(&context, target)
    }

    /// Explicitly pulls a new selected view; this wrapper continues to describe its original snapshot.
    pub fn pull(&self) -> Result<Self, HolonError> {
        let target = self.described_holon_type()?.holon().clone();
        let context = self.holon.resolution_context()?;
        context.get_holon_service().pull_effective_holon_descriptor_internal(&context, target)
    }

    /// Reads this instance's Commit-local prepared contract without initiating construction.
    /// Persistence uses this after assessment, when all candidate decisions are already fixed.
    pub fn find_for_instance(source: &HolonReference) -> Result<Option<Self>, HolonError> {
        let context = source.resolution_context()?;
        let descriptor = source.holon_descriptor()?;
        Self::find_in_context(&context, descriptor.holon())
    }

    /// Reuses published products during structural assembly without starting construction.
    pub(crate) fn find_prepared(target: &HolonReference) -> Result<Option<Self>, HolonError> {
        let context = target.resolution_context()?;
        if let Some(descriptor) = Self::find_in_context(&context, target)? {
            return Ok(Some(descriptor));
        }
        if matches!(target, HolonReference::Smart(_)) {
            if let Some(registry) = context.descriptor_registry() {
                return registry.get(&target.holon_id()?);
            }
        }
        Ok(None)
    }

    /// Finds saved entries in a Commit-local snapshot without following Extends.
    pub fn find_in_context(
        context: &Arc<TransactionContext>,
        target: &HolonReference,
    ) -> Result<Option<Self>, HolonError> {
        if !matches!(target, HolonReference::Smart(_)) {
            return Ok(None);
        }
        let Some(root) = context.descriptor_registry_root()? else {
            return Ok(None);
        };
        let members = root
            .related_holons(CoreRelationshipTypeName::PreparedDescriptors)?
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
            .get_members()
            .clone();
        for member in members {
            let descriptor = Self::from_constructed_holon(member);
            if same_definition(descriptor.described_holon_type()?.holon(), target)? {
                return Ok(Some(descriptor));
            }
        }
        Ok(None)
    }

    /// Filters the prepared outbound vocabulary by this instance's lifecycle state.
    pub fn available_relationships(
        &self,
        source: &HolonReference,
    ) -> Result<Vec<RelationshipDescriptor>, HolonError> {
        Ok(crate::reference_layer::available_relationships::available_relationships_from_effective(
            source, self,
        )?
        .into_iter()
        .map(|entry| entry.descriptor)
        .collect())
    }

    pub fn allows_property(
        &self,
        name: impl type_names::ToPropertyName,
    ) -> Result<PropertyDescriptor, HolonError> {
        self.get_property_by_name(name)
    }
    pub fn affords_dance(
        &self,
        name: impl type_names::ToDanceName,
    ) -> Result<DanceDescriptor, HolonError> {
        self.get_dance_by_name(name)
    }
    pub fn affords_command(
        &self,
        name: impl type_names::ToCommandName,
    ) -> Result<CommandDescriptor, HolonError> {
        self.get_command_by_name(name)
    }

    pub fn get_property_by_name(
        &self,
        name: impl type_names::ToPropertyName,
    ) -> Result<PropertyDescriptor, HolonError> {
        let name = name.to_property_name();
        for descriptor in self.instance_properties()? {
            if descriptor.property_name()? == name {
                return Ok(descriptor);
            }
        }
        Err(self.missing("property", name.to_string()))
    }

    pub fn get_relationship_by_name(
        &self,
        name: impl ToRelationshipName,
    ) -> Result<RelationshipDescriptor, HolonError> {
        let name = name.to_relationship_name();
        for descriptor in self.navigable_relationships()? {
            if descriptor.base_relationship_name()? == name {
                return Ok(descriptor);
            }
        }
        Err(self.missing("relationship", name.to_string()))
    }

    pub fn get_dance_by_name(
        &self,
        name: impl type_names::ToDanceName,
    ) -> Result<DanceDescriptor, HolonError> {
        let name = name.to_dance_name();
        for descriptor in self.afforded_dances()? {
            if descriptor.dance_name()? == name {
                return Ok(descriptor);
            }
        }
        Err(self.missing("dance", name.to_string()))
    }

    pub fn get_command_by_name(
        &self,
        name: impl type_names::ToCommandName,
    ) -> Result<CommandDescriptor, HolonError> {
        let name = name.to_command_name();
        for descriptor in self.afforded_commands()? {
            if descriptor.command_name()? == name {
                return Ok(descriptor);
            }
        }
        Err(self.missing("command", name.to_string()))
    }

    fn missing(&self, kind: &str, name: String) -> HolonError {
        HolonError::DescriptorDeclarationNotFound {
            kind: kind.into(),
            name,
            descriptor: self
                .described_holon_type()
                .map(|descriptor| accessor_helpers::descriptor_label(descriptor.holon()))
                .unwrap_or_else(|_| self.holon.reference_id_string()),
        }
    }

    /// Wraps the successful result of the typed construction service without revalidation.
    /// Arbitrary references must use `try_from_holon` instead.
    pub fn from_constructed_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Narrows a readable transient result by its describing type.
    pub fn try_from_holon(holon: HolonReference) -> Result<Self, HolonError> {
        require_transient_type(&holon, CoreHolonTypeName::EffectiveHolonDescriptor)?;
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
    pub fn navigable_relationships(&self) -> Result<Vec<RelationshipDescriptor>, HolonError> {
        Ok(targets(&self.holon, CoreRelationshipTypeName::NavigableRelationships)?
            .into_iter()
            .map(RelationshipDescriptor::from_holon)
            .collect())
    }

    pub fn prepared_schema(&self) -> Result<PreparedSchema, HolonError> {
        PreparedSchema::read(&self.holon)
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
        let contributions = self.contributions()?;
        // Construction leaves this collection empty. There is no claimed
        // Holon-based occurrence sequence to compare with the direct products.
        if contributions.is_empty() {
            return Ok(());
        }
        let context = self.holon.resolution_context()?;
        let mut grouped = std::collections::HashMap::<String, Vec<(i64, HolonReference)>>::new();
        for record in contributions {
            record.validate_provenance()?;
            let name = record.collection()?.base_relationship_name()?.to_string();
            grouped.entry(name).or_default().push((record.position()?, record.member()?.member));
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
            let expected = records
                .into_iter()
                .map(|(_, member)| DefinitionIdentity::for_reference(&member, &context))
                .collect::<Result<Vec<_>, _>>()?;
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
pub fn construct_effective_holon_descriptor(
    context: &Arc<TransactionContext>,
    target: HolonReference,
) -> Result<EffectiveHolonDescriptor, HolonError> {
    construct_effective_holon_descriptor_impl(context, target, |_, _| {})
}

/// Controlled profiling entry point for the reference-oriented projection.
#[cfg(feature = "descriptor-performance-profiling")]
pub fn profile_effective_holon_descriptor_construction(
    context: &Arc<TransactionContext>,
    target: HolonReference,
    checkpoint: impl FnMut(&str, &str),
) -> Result<EffectiveHolonDescriptor, HolonError> {
    construct_effective_holon_descriptor_impl(context, target, checkpoint)
}

fn construct_effective_holon_descriptor_impl(
    context: &Arc<TransactionContext>,
    target: HolonReference,
    mut checkpoint: impl FnMut(&str, &str),
) -> Result<EffectiveHolonDescriptor, HolonError> {
    let computed = ComputedEffectiveSchema::resolve_with_checkpoint(&target, &mut checkpoint)?;
    materialize_effective_schema(context, &computed, checkpoint)
}

/// Materializes an already computed projection without traversing its lineage again.
pub(super) fn materialize_effective_schema(
    context: &Arc<TransactionContext>,
    computed: &ComputedEffectiveSchema,
    mut checkpoint: impl FnMut(&str, &str),
) -> Result<EffectiveHolonDescriptor, HolonError> {
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
        &CoreHolonTypeName::EffectiveHolonDescriptor
            .descriptor_key(&CoreHolonTypeName::HolonType)
            .0,
    )?;
    let owner = context.get_space_holon()?;
    let mut result = context.mutation().new_holon(None)?;
    result.add_related_references(CoreRelationshipTypeName::DescribedBy, vec![result_type])?;
    if let Some(space) = &owner {
        result.add_related_references(CoreRelationshipTypeName::OwnedBy, vec![space.clone()])?;
    }
    result.add_related_references(CoreRelationshipTypeName::DescribedHolonType, vec![target])?;
    result.add_related_references(CoreRelationshipTypeName::NavigableRelationships, navigable)?;
    checkpoint("result_allocation", "");
    for collection in &computed.collections {
        let result_name = collection.result.clone();
        let contributions = &collection.occurrences;
        let values = contributions.iter().map(|entry| entry.member.clone()).collect();
        let result_label = result_name.as_relationship_name().to_string();
        result.add_related_references(result_name, values)?;
        checkpoint("product_population", &result_label);
    }
    result.add_related_references(CoreRelationshipTypeName::EffectiveContributions, vec![])?;
    let prepared = PreparedSchema::compute(computed)?;
    result.with_property_value(
        CorePropertyTypeName::PreparedSchema,
        MapString(
            serde_json::to_string(&prepared)
                .map_err(|error| HolonError::InvalidParameter(error.to_string()))?,
        ),
    )?;
    result.mark_as_immutable()?;
    let result = Ok(EffectiveHolonDescriptor { holon: result.into() });
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
    use crate::{HolonCollectionApi, WritableHolon};

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
            &CoreHolonTypeName::EffectiveHolonDescriptor
                .descriptor_key(&CoreHolonTypeName::HolonType)
                .0,
            "EffectiveHolonDescriptor",
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
        let result = construct_effective_holon_descriptor(&context, target.clone())?;
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
        assert!(contributions.is_empty());
        assert_eq!(inheritance::lineage_edge_reads(&target) - target_before, 1);
        assert_eq!(inheritance::lineage_edge_reads(&parent) - parent_before, 1);
        let mut reference = result.holon().clone();
        assert!(reference.with_property_value("Corrupt", true).is_err());
        let collection = reference.related_holons("EffectiveContributions")?;
        assert!(collection.write().unwrap().add_references(vec![target.clone()]).is_err());
        let second = construct_effective_holon_descriptor(&context, target)?;
        assert!(!same_definition(result.holon(), second.holon())?);
        context.dispose()?;
        assert!(result.effective_key_rule().is_err());
        Ok(())
    }

    #[test]
    fn computed_provenance_preserves_repeated_occurrences_without_holons() -> Result<(), HolonError>
    {
        let (context, mut parent, mut child) = fixture()?;
        let mut constraint = new_test_holon(&context, "constraint")?;
        constraint.with_descriptor(resolve_core_descriptor(&context, "Constraint.HolonType")?)?;
        let constraint: HolonReference = context.mutation().stage_new_holon(constraint)?.into();
        parent
            .add_related_holons(CoreRelationshipTypeName::Constraints, vec![constraint.clone()])?;
        child.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![constraint.clone(), constraint.clone()],
        )?;
        let computed = ComputedEffectiveSchema::resolve(&child)?;
        let occurrences = computed.contributions(CoreRelationshipTypeName::Constraints);
        assert_eq!(occurrences.len(), 3);
        for (index, occurrence) in occurrences.iter().enumerate() {
            assert!(same_definition(&occurrence.member, &constraint)?);
            assert!(same_definition(
                &occurrence.declared_on,
                if index == 0 { &parent } else { &child }
            )?);
            assert_eq!(occurrence.local_position, [0, 0, 1][index]);
        }
        let before = context.lookup().transient_count()?;
        let result = computed.materialize(&context)?;
        assert_eq!(context.lookup().transient_count()? - before, 1);
        assert_eq!(result.effective_constraints()?.len(), 3);
        assert!(
            targets(result.holon(), CoreRelationshipTypeName::EffectiveContributions)?.is_empty()
        );
        Ok(())
    }

    #[test]
    fn products_preserve_repeated_constraint_and_binding_declarations() -> Result<(), HolonError> {
        let (context, mut parent, mut child) = fixture()?;
        for (source, category) in [
            (CoreRelationshipTypeName::Constraints, "Constraint.HolonType"),
            (CoreRelationshipTypeName::ValidationBindings, "ValidationRule.HolonType"),
        ] {
            let (_, _, key, _, _) = collections()
                .into_iter()
                .find(|(name, ..)| {
                    name.clone().to_relationship_name() == source.clone().to_relationship_name()
                })
                .unwrap();
            let mut source_descriptor = resolve_core_descriptor(&context, &key.0)?;
            source_descriptor.with_property_value("AllowsDuplicates", false)?;
            let mut member = new_test_holon(&context, &format!("{source:?}-member"))?;
            member.with_descriptor(resolve_core_descriptor(&context, category)?)?;
            let member: HolonReference = context.mutation().stage_new_holon(member)?.into();
            parent.add_related_holons(source.clone(), vec![member.clone()])?;
            child.add_related_holons(source, vec![member.clone(), member])?;
        }
        let computed = ComputedEffectiveSchema::resolve(&child)?;
        let before = context.lookup().transient_count()?;
        let result = computed.materialize(&context)?;
        assert_eq!(context.lookup().transient_count()? - before, 1);
        assert_eq!(result.effective_constraints()?.len(), 3);
        assert_eq!(result.effective_validation_bindings()?.len(), 3);
        assert!(
            targets(result.holon(), CoreRelationshipTypeName::EffectiveContributions)?.is_empty()
        );
        for source in
            [CoreRelationshipTypeName::Constraints, CoreRelationshipTypeName::ValidationBindings]
        {
            let occurrences = computed.contributions(source);
            assert_eq!(occurrences.len(), 3);
            for (index, occurrence) in occurrences.iter().enumerate() {
                assert_eq!(occurrence.local_position, [0, 0, 1][index]);
                assert!(same_definition(
                    &occurrence.declared_on,
                    if index == 0 { &parent } else { &child }
                )?);
            }
        }
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
                    member.with_property_value(CorePropertyTypeName::IsDefinitional, true)?;
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
        let computed = ComputedEffectiveSchema::resolve(&child)?;
        let result = computed.materialize(&context)?;

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
        assert!(
            targets(result.holon(), CoreRelationshipTypeName::EffectiveContributions)?.is_empty()
        );
        let key_rule = result.effective_key_rule()?.holon().clone();
        for (source, ..) in collections() {
            let occurrences = computed.contributions(source);
            assert_eq!(occurrences.len(), 1);
            let occurrence = &occurrences[0];
            assert!(same_definition(
                &occurrence.declared_on,
                if same_definition(&occurrence.member, &key_rule)? { &child } else { &parent }
            )?);
            assert_eq!(occurrence.local_position, 0);
        }
        Ok(())
    }

    #[test]
    fn staged_child_with_no_materialized_inverse_index_constructs() -> Result<(), HolonError> {
        let (context, _, child) = fixture()?;
        let HolonReference::Transient(child) = child else { unreachable!() };
        let child = context.mutation().stage_new_holon(child)?;
        let result = construct_effective_holon_descriptor(&context, child.into())?;
        assert!(result.navigable_relationships()?.is_empty());
        Ok(())
    }

    #[test]
    fn cycles_fail_but_invalid_key_rule_cardinality_remains_projectable() -> Result<(), HolonError>
    {
        let (context, _, mut child) = fixture()?;
        let mut cycle = new_test_holon(&context, "cycle")?;
        cycle.add_related_holons(CoreRelationshipTypeName::Extends, vec![cycle.clone().into()])?;
        assert!(construct_effective_holon_descriptor(&context, cycle.into()).is_err());
        child.add_related_holons(
            CoreRelationshipTypeName::InstanceKeyRule,
            vec![
                resolve_core_descriptor(&context, "NoneRule.KeyRuleType")?,
                canonical(&context, "OtherRule.KeyRuleType", "OtherRule")?,
            ],
        )?;
        let result = construct_effective_holon_descriptor(&context, child)?;
        assert_eq!(
            targets(result.holon(), CoreRelationshipTypeName::EffectiveInstanceKeyRule)?.len(),
            2
        );
        assert!(result.effective_key_rule().is_err());
        let root = resolve_core_descriptor(&context, "HolonType.TypeDescriptor")?;
        let result = construct_effective_holon_descriptor(&context, root)?;
        assert!(
            targets(result.holon(), CoreRelationshipTypeName::EffectiveInstanceKeyRule)?.is_empty()
        );
        assert!(result.effective_key_rule().is_err());
        Ok(())
    }
}
