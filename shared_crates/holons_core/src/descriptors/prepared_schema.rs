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
        for occurrence in computed.navigable_relationships()? {
            let relationship = RelationshipDescriptor::from_holon(occurrence.member.clone());
            let updateable = super::classify_relationship_direction(relationship.holon())?
                == RelationshipDirection::Declared;
            let policy = if updateable && relationship.is_definitional()? {
                RelationshipCachePolicy::Reuse
            } else {
                match relationship.membership_cache_max_age_millis()? {
                    0 => RelationshipCachePolicy::Fresh,
                    age => RelationshipCachePolicy::MaxAgeMillis(age),
                }
            };
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
