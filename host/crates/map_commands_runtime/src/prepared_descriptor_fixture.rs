//! Service-boundary fixture for command tests; real schema construction is covered by guest integration.
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::descriptors::{PreparedRelationship, PreparedSchema};
use holons_core::{
    ComputedEffectiveSchema, Descriptor, EffectiveHolonDescriptor, HolonReference, WritableHolon,
};
use std::sync::Arc;
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

pub(crate) fn construct(
    context: &Arc<TransactionContext>,
    target: HolonReference,
) -> Result<EffectiveHolonDescriptor, HolonError> {
    let schema = ComputedEffectiveSchema::resolve(&target)?;
    let mut result = context.mutation().new_holon(None)?;
    result.add_related_holons(CoreRelationshipTypeName::DescribedHolonType, vec![target])?;
    for (source, product) in [
        (
            CoreRelationshipTypeName::InstanceProperties,
            CoreRelationshipTypeName::EffectiveInstanceProperties,
        ),
        (
            CoreRelationshipTypeName::InstanceRelationships,
            CoreRelationshipTypeName::EffectiveInstanceRelationships,
        ),
        (CoreRelationshipTypeName::AffordsDance, CoreRelationshipTypeName::EffectiveAffordsDance),
        (
            CoreRelationshipTypeName::AffordsCommand,
            CoreRelationshipTypeName::EffectiveAffordsCommand,
        ),
        (
            CoreRelationshipTypeName::ValidationBindings,
            CoreRelationshipTypeName::EffectiveValidationBindings,
        ),
        (CoreRelationshipTypeName::Constraints, CoreRelationshipTypeName::EffectiveConstraints),
        (
            CoreRelationshipTypeName::InstanceKeyRule,
            CoreRelationshipTypeName::EffectiveInstanceKeyRule,
        ),
    ] {
        result.add_related_holons(
            product,
            schema.contributions(source).iter().map(|entry| entry.member.clone()).collect(),
        )?;
    }
    let navigation = schema.navigable_relationships()?;
    result.add_related_holons(
        CoreRelationshipTypeName::PluralNavigableRelationships,
        navigation.iter().map(|entry| entry.member.clone()).collect(),
    )?;
    let declared = schema
        .instance_relationships()
        .iter()
        .map(|entry| entry.holon().full_reference_id_string())
        .collect::<std::collections::HashSet<_>>();
    let relationships = navigation
        .iter()
        .map(|entry| {
            let descriptor = holons_core::RelationshipDescriptor::from_holon(entry.member.clone());
            Ok(PreparedRelationship {
                identity: entry.member.full_reference_id_string(),
                name: descriptor.base_relationship_name()?,
                policy: holons_core::RelationshipCachePolicy::Fresh,
                updateable: declared.contains(&entry.member.full_reference_id_string()),
            })
        })
        .collect::<Result<_, HolonError>>()?;
    let prepared = PreparedSchema { relationships, ..Default::default() };
    result.with_property_value(
        CorePropertyTypeName::PreparedSchema,
        base_types::MapString(serde_json::to_string(&prepared).unwrap()),
    )?;
    result.mark_as_immutable()?;
    Ok(EffectiveHolonDescriptor::from_constructed_holon(result.into()))
}
