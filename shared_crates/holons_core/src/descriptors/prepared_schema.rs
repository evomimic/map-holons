//! Compact prepared metadata; contains identities and values, never bound handles.
use super::{ComputedEffectiveSchema, Descriptor, RelationshipDescriptor, RelationshipDirection};
use crate::{HolonReference, ReadableHolon, RelationshipCachePolicy};
use core_types::{HolonError, HolonId, RelationshipName};
use serde::{Deserialize, Serialize};
use type_names::CorePropertyTypeName;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedRelationship {
    pub identity: String,
    pub name: RelationshipName,
    /// Scalar projection acquired through RelationshipDescriptor in the construction guest.
    pub policy: RelationshipCachePolicy,
    pub updateable: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PreparedSchema {
    pub relationships: Vec<PreparedRelationship>,
    pub lineage: Vec<HolonId>,
    pub navigation_origins: Vec<(HolonId, usize, bool)>,
    /// Full source navigation identities, including maximum-zero declarations, aligned with origins.
    pub navigation_members: Vec<HolonId>,
    /// Declaring type and local position, in the same order as each effective product.
    pub origins: Vec<Vec<(HolonId, usize)>>,
}
impl PreparedSchema {
    pub(super) fn compute(computed: &ComputedEffectiveSchema) -> Result<Self, HolonError> {
        let mut prepared = Self::default();
        let mut policies = std::collections::HashSet::new();
        // A meta-type's explicit declared contract licenses its descriptor
        // instances even when endpoint filtering omits it from that meta-type's
        // own navigation. Policy metadata must retain those exact declarations.
        for occurrence in computed
            .contributions(type_names::CoreRelationshipTypeName::InstanceRelationships)
            .iter()
            .chain(computed.navigable_relationships()?.iter())
        {
            if !policies.insert(occurrence.member.full_reference_id_string()) {
                continue;
            }
            let relationship = RelationshipDescriptor::from_holon(occurrence.member.clone());
            let updateable = relationship.direction()? == RelationshipDirection::Declared;
            let policy = relationship.membership_cache_policy()?;
            prepared.relationships.push(PreparedRelationship {
                identity: relationship.holon().full_reference_id_string(),
                name: relationship.base_relationship_name()?,
                policy,
                updateable,
            });
        }
        if computed.lineage.iter().all(|reference| matches!(reference, HolonReference::Smart(_))) {
            prepared.navigation_members = computed
                .navigable_relationships()?
                .iter()
                .map(|entry| entry.member.holon_id())
                .collect::<Result<_, _>>()?;
            prepared.navigation_origins = computed
                .navigable_relationships()?
                .iter()
                .map(|entry| {
                    Ok((
                        entry.declared_on.holon_id()?,
                        entry.local_position,
                        matches!(
                            entry.source_collection,
                            type_names::CoreRelationshipTypeName::SourceOf
                        ),
                    ))
                })
                .collect::<Result<_, HolonError>>()?;
            prepared.lineage =
                computed.lineage.iter().map(ReadableHolon::holon_id).collect::<Result<_, _>>()?;
            prepared.origins = computed
                .collections
                .iter()
                .map(|collection| {
                    collection
                        .occurrences
                        .iter()
                        .map(|occurrence| {
                            Ok((occurrence.declared_on.holon_id()?, occurrence.local_position))
                        })
                        .collect::<Result<Vec<_>, HolonError>>()
                })
                .collect::<Result<_, _>>()?;
        }
        Ok(prepared)
    }

    pub fn read(reference: &HolonReference) -> Result<Self, HolonError> {
        match reference.property_value(CorePropertyTypeName::PreparedSchema)? {
            Some(base_types::BaseValue::StringValue(value)) => serde_json::from_str(&value.0)
                .map_err(|error| HolonError::InvalidParameter(error.to_string())),
            _ => Err(HolonError::InvalidParameter("Missing prepared schema metadata".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, new_declared_relationship_descriptor_holon, new_holon_type_descriptor,
    };
    use crate::WritableHolon;
    use type_names::CoreRelationshipTypeName as R;

    #[test]
    fn policy_projection_keeps_explicit_contract_members_outside_meta_type_endpoint_lineage(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let endpoint =
            new_holon_type_descriptor(&context, "relationship-root", "RelationshipType")?;
        let mut meta_type =
            new_holon_type_descriptor(&context, "meta-relationship", "MetaRelationshipType")?;
        let mut declared = new_declared_relationship_descriptor_holon(
            &context,
            "source-type",
            "SourceType",
            endpoint.clone().into(),
            endpoint.into(),
        )?;
        declared.with_property_value("IsDefinitional", true)?;
        meta_type.add_related_holons(R::InstanceRelationships, vec![declared.clone().into()])?;
        let computed = ComputedEffectiveSchema::resolve(&meta_type.into())?;
        assert!(computed.navigable_relationships()?.is_empty());
        let prepared = PreparedSchema::compute(&computed)?;
        assert_eq!(prepared.relationships.len(), 1);
        assert_eq!(
            prepared.relationships[0].identity,
            HolonReference::from(declared).full_reference_id_string()
        );
        assert_eq!(prepared.relationships[0].policy, RelationshipCachePolicy::Reuse);
        assert!(prepared.relationships[0].updateable);
        Ok(())
    }
}
