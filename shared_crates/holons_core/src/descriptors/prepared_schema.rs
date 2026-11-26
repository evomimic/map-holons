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
    /// Restrictive HolonType lifecycle permission, computed from the same lineage.
    /// Readable malformed declarations retain their error; storage failures abort preparation.
    #[serde(default)]
    pub instance_deletion_allowed: Option<Result<bool, HolonError>>,
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
        let mut prepared = Self {
            instance_deletion_allowed: Some(prepare_instance_deletion_permission(
                &computed.lineage,
            )?),
            ..Self::default()
        };
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

/// Computes the restrictive permission without resolving another Extends edge.
/// The outer Result distinguishes operational failures from malformed declaration values.
pub(crate) fn prepare_instance_deletion_permission(
    lineage: &[HolonReference],
) -> Result<Result<bool, HolonError>, HolonError> {
    for descriptor in lineage {
        match descriptor.property_value(CorePropertyTypeName::InstanceDeletionAllowed)? {
            Some(base_types::BaseValue::BooleanValue(value)) if !value.0 => return Ok(Ok(false)),
            Some(base_types::BaseValue::BooleanValue(_)) => {}
            Some(other) => {
                return Ok(Err(HolonError::UnexpectedValueType(
                    format!("{other:?}"),
                    "Boolean".into(),
                )));
            }
            None => {
                return Ok(Err(HolonError::InvalidState(
                    "InstanceDeletionAllowed is missing from a holon type descriptor".into(),
                )));
            }
        }
    }
    Ok(Ok(true))
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
    fn deletion_projection_preserves_restriction_and_field_errors() -> Result<(), HolonError> {
        use crate::descriptors::test_support::new_test_holon;
        let context = build_context();
        let mut child = new_test_holon(&context, "child")?;
        let mut parent = new_test_holon(&context, "parent")?;
        child.with_property_value(CorePropertyTypeName::InstanceDeletionAllowed, true)?;
        parent.with_property_value(CorePropertyTypeName::InstanceDeletionAllowed, false)?;
        assert_eq!(
            prepare_instance_deletion_permission(&[child.clone().into(), parent.clone().into()])?,
            Ok(false)
        );
        parent.with_property_value(CorePropertyTypeName::InstanceDeletionAllowed, true)?;
        assert_eq!(
            prepare_instance_deletion_permission(&[child.clone().into(), parent.clone().into()])?,
            Ok(true)
        );
        parent.with_property_value(CorePropertyTypeName::InstanceDeletionAllowed, "invalid")?;
        let malformed = prepare_instance_deletion_permission(&[child.into(), parent.into()])?;
        assert!(matches!(malformed, Err(HolonError::UnexpectedValueType(_, _))));
        let missing = new_test_holon(&context, "missing")?;
        assert!(matches!(
            prepare_instance_deletion_permission(&[missing.into()])?,
            Err(HolonError::InvalidState(_))
        ));
        let prepared = PreparedSchema {
            instance_deletion_allowed: Some(malformed.clone()),
            ..Default::default()
        };
        let restored: PreparedSchema =
            serde_json::from_str(&serde_json::to_string(&prepared).unwrap()).unwrap();
        assert_eq!(restored.instance_deletion_allowed, Some(malformed));
        Ok(())
    }

    #[test]
    fn deletion_projection_does_not_cache_operational_errors_as_field_errors(
    ) -> Result<(), HolonError> {
        use crate::descriptors::test_support::build_context_with_read_errors;
        use core_types::{HolonId, LocalId};
        let id = HolonId::Local(LocalId(vec![71; 39]));
        // Even an error shaped like a semantic field failure originated from I/O here.
        let error = HolonError::UnexpectedValueType("backend error".into(), "Boolean".into());
        let context = build_context_with_read_errors(
            vec![],
            Default::default(),
            std::collections::HashMap::from([(id.clone(), error.clone())]),
        );
        let reference = HolonReference::smart_from_id(context.space_read_handle(), id);
        assert_eq!(prepare_instance_deletion_permission(&[reference]), Err(error));
        Ok(())
    }

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
