use std::sync::RwLockReadGuard;

use crate::core_shared_objects::HolonCollection;
use crate::descriptors::{walk_extends_chain, HolonDescriptor, TypeHeader};
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, RelationshipName};
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, ToPropertyName};

/// Returns a required string property from a descriptor holon.
pub(crate) fn require_string<T: ToPropertyName>(
    holon: &HolonReference,
    property_name: T,
) -> Result<MapString, HolonError> {
    let name = property_name.to_property_name();
    match holon.property_value(&name)? {
        Some(BaseValue::StringValue(value)) => Ok(value),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{:?}", other), "String".to_string()))
        }
        None => Err(HolonError::EmptyField(name.to_string())),
    }
}

/// Returns an optional string-like property from a descriptor holon.
///
/// Enum values are accepted because several schema-backed structural values are
/// represented as enum value descriptors while callers need their string names.
pub(crate) fn optional_string<T: ToPropertyName>(
    holon: &HolonReference,
    property_name: T,
) -> Result<Option<MapString>, HolonError> {
    let name = property_name.to_property_name();
    match holon.property_value(&name)? {
        Some(BaseValue::StringValue(value)) => Ok(Some(value)),
        Some(BaseValue::EnumValue(value)) => Ok(Some(value.0)),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{:?}", other), "String".to_string()))
        }
        None => Ok(None),
    }
}

/// Returns a required boolean property from a descriptor holon.
pub(crate) fn require_bool<T: ToPropertyName>(
    holon: &HolonReference,
    property_name: T,
) -> Result<bool, HolonError> {
    let name = property_name.to_property_name();
    match holon.property_value(&name)? {
        Some(BaseValue::BooleanValue(value)) => Ok(value.0),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{:?}", other), "Boolean".to_string()))
        }
        None => Err(HolonError::EmptyField(name.to_string())),
    }
}

/// Returns a required integer property from a descriptor holon.
pub(crate) fn require_integer<T: ToPropertyName>(
    holon: &HolonReference,
    property_name: T,
) -> Result<i64, HolonError> {
    let name = property_name.to_property_name();
    match holon.property_value(&name)? {
        Some(BaseValue::IntegerValue(value)) => Ok(value.0),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{:?}", other), "Integer".to_string()))
        }
        None => Err(HolonError::EmptyField(name.to_string())),
    }
}

/// Returns the single target of a required singular descriptor relationship.
pub(crate) fn require_single_related(
    holon: &HolonReference,
    relationship_name: CoreRelationshipTypeName,
) -> Result<HolonReference, HolonError> {
    let relationship = relationship_name.as_relationship_name().to_string();
    let collection_arc = holon.related_holons(relationship_name)?;
    let collection = collection_arc.read().map_err(lock_error)?;
    let members = collection.get_members();

    match members.as_slice() {
        [] => Err(HolonError::MissingRequiredRelationship {
            relationship,
            descriptor: descriptor_label(holon),
        }),
        [single] => Ok(single.clone()),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship,
            descriptor: descriptor_label(holon),
            count: many.len(),
        }),
    }
}

/// Returns the optional target of a singular descriptor relationship.
pub(crate) fn optional_single_related(
    holon: &HolonReference,
    relationship_name: CoreRelationshipTypeName,
) -> Result<Option<HolonReference>, HolonError> {
    let relationship = relationship_name.as_relationship_name().to_string();
    let collection_arc = holon.related_holons(relationship_name)?;
    let collection = collection_arc.read().map_err(lock_error)?;
    let members = collection.get_members();

    match members.as_slice() {
        [] => Ok(None),
        [single] => Ok(Some(single.clone())),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship,
            descriptor: descriptor_label(holon),
            count: many.len(),
        }),
    }
}

/// Searches a descriptor's effective `Extends` chain for a matching type name.
pub(crate) fn search_extends_chain<T>(
    holon: &HolonReference,
    expected_type_names: &[MapString],
    matcher: impl Fn(&MapString) -> Option<T>,
) -> Result<T, HolonError> {
    let mut found = None;

    // Walk the effective type lineage self-first.
    for ancestor in walk_extends_chain(holon) {
        let ancestor = ancestor?;
        match TypeHeader::new(&ancestor).type_name() {
            Ok(type_name) => {
                if found.is_none() {
                    found = Some(type_name.to_string());
                }
                if let Some(match_result) = matcher(&type_name) {
                    return Ok(match_result);
                }
            }
            Err(_) => {
                if found.is_none() {
                    found = Some(descriptor_label(&ancestor));
                }
            }
        }
    }

    // Preserve the established descriptor-kind diagnostic shape.
    Err(HolonError::WrongDescriptorKind {
        expected: format_expected_type_names(expected_type_names),
        found: found.unwrap_or_else(|| "unknown".to_string()),
        descriptor: descriptor_label(holon),
    })
}

fn format_expected_type_names(expected_type_names: &[MapString]) -> String {
    match expected_type_names {
        [] => "unknown".to_string(),
        [single] => single.to_string(),
        [first, second] => format!("{first} or {second}"),
        many => {
            let names = many.iter().map(ToString::to_string).collect::<Vec<_>>();
            let (last, leading) =
                names.split_last().expect("non-empty expected names should split");
            format!("{}, or {}", leading.join(", "), last)
        }
    }
}

/// Validates that a descriptor's effective `Extends` chain reaches the expected type name.
pub(crate) fn validate_extends_chain_reaches(
    holon: &HolonReference,
    expected_type_name: &MapString,
) -> Result<(), HolonError> {
    search_extends_chain(holon, std::slice::from_ref(expected_type_name), |type_name| {
        (type_name == expected_type_name).then_some(())
    })
}

/// Returns whether the relationship participates in defining identity or structure.
pub(crate) fn relationship_is_definitional(holon: &HolonReference) -> Result<bool, HolonError> {
    require_bool(holon, CorePropertyTypeName::IsDefinitional)
}

/// Returns whether related members have schema-significant order.
pub(crate) fn relationship_is_ordered(holon: &HolonReference) -> Result<bool, HolonError> {
    require_bool(holon, CorePropertyTypeName::IsOrdered)
}

/// Returns whether repeated target references are allowed.
pub(crate) fn relationship_allows_duplicates(holon: &HolonReference) -> Result<bool, HolonError> {
    require_bool(holon, CorePropertyTypeName::AllowsDuplicates)
}

/// Returns the minimum number of targets permitted by a relationship.
pub(crate) fn relationship_min_cardinality(holon: &HolonReference) -> Result<i64, HolonError> {
    require_integer(holon, CorePropertyTypeName::MinCardinality)
}

/// Returns the maximum number of targets permitted by a relationship.
pub(crate) fn relationship_max_cardinality(holon: &HolonReference) -> Result<i64, HolonError> {
    require_integer(holon, CorePropertyTypeName::MaxCardinality)
}

/// Returns the optional deletion semantic declared by a relationship, when populated.
pub(crate) fn relationship_deletion_semantic(
    holon: &HolonReference,
) -> Result<Option<MapString>, HolonError> {
    optional_string(holon, CorePropertyTypeName::DeletionSemantic)
}

/// Returns a relationship descriptor's base relationship name.
pub(crate) fn relationship_base_relationship_name(
    holon: &HolonReference,
) -> Result<RelationshipName, HolonError> {
    Ok(RelationshipName(TypeHeader::new(holon).type_name()?))
}

/// Returns the source holon descriptor reached through required `SourceType`.
pub(crate) fn relationship_source_type(
    holon: &HolonReference,
) -> Result<HolonDescriptor, HolonError> {
    let source_type = require_single_related(holon, CoreRelationshipTypeName::SourceType)?;
    Ok(HolonDescriptor::from_holon(source_type))
}

/// Returns the target holon descriptor reached through required `TargetType`.
pub(crate) fn relationship_target_type(
    holon: &HolonReference,
) -> Result<HolonDescriptor, HolonError> {
    let target_type = require_single_related(holon, CoreRelationshipTypeName::TargetType)?;
    Ok(HolonDescriptor::from_holon(target_type))
}

/// Returns the full `(Source)-[Base]->(Target)` relationship name.
pub(crate) fn relationship_full_relationship_name(
    holon: &HolonReference,
) -> Result<MapString, HolonError> {
    let source_name = relationship_source_type(holon)?.header().type_name()?;
    let base_name = relationship_base_relationship_name(holon)?;
    let target_name = relationship_target_type(holon)?.header().type_name()?;

    Ok(MapString(format!("({source_name})-[{base_name}]->({target_name})")))
}

/// Best-effort descriptor label for structural descriptor errors.
///
/// Prefer the human-readable summary when available, but fall back to the
/// stable reference id so error construction never cascades into a second
/// failure path.
pub(crate) fn descriptor_label(holon: &HolonReference) -> String {
    match holon.summarize() {
        Ok(summary) => summary,
        Err(_) => holon.reference_id_string(),
    }
}

/// Normalizes poisoned collection-lock errors into the crate's standard
/// `FailedToAcquireLock` surface.
pub(crate) fn lock_error(
    error: std::sync::PoisonError<RwLockReadGuard<'_, HolonCollection>>,
) -> HolonError {
    HolonError::FailedToAcquireLock(format!(
        "Failed to acquire read lock on holon collection: {}",
        error
    ))
}
