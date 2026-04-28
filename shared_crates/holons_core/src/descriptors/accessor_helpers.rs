use std::sync::RwLockReadGuard;

use crate::core_shared_objects::HolonCollection;
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::{CoreRelationshipTypeName, ToPropertyName};

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
