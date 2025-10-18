use std::sync::{Arc, RwLock};

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, LocalId, PropertyMap, PropertyName, PropertyValue,
    RelationshipName,
};
use derive_new::new;
use serde::{Deserialize, Serialize};

use super::state::AccessType;
use super::{SavedHolon, StagedHolon, TransientHolon};
use crate::core_shared_objects::holon::EssentialHolonContent;
use crate::core_shared_objects::holon_behavior::{ReadableHolonState, WriteableHolonState};
use crate::{HolonCollection, HolonReference, HolonsContextBehavior, RelationshipMap};

/// Enum representing the three Holon phases: `Transient`, `Staged`, and `Saved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Holon {
    Transient(TransientHolon),
    Staged(StagedHolon),
    Saved(SavedHolon),
}

/// A normalized, serializable representation of a `Holon`'s essential state,
/// used specifically for cloning operations.
///
/// `HolonCloneModel` exists to bridge across the internal differences of the
/// various `Holon` variants (`TransientHolon`, `StagedHolon`, `SavedHolon`)
/// by capturing just the common fields needed to construct a new
/// `TransientHolon` clone of any source holon.
///
/// This model:
/// - Records the `version` to preserve lineage and hashing context.
/// - Optionally tracks the `original_id` of the holon being cloned, if it
///   originated from a persisted (`SavedHolon`) instance.
/// - Copies over the holon’s `properties`, which form the self-describing
///   property data.
/// - Optionally includes the `relationships` (transient, staged, or saved,
///   normalized to a common form).
///
/// By design, this type is decoupled from the richer state and invariants of
/// the `Holon` variants (e.g. `HolonState`, `ValidationState`, errors,
/// commit/staging metadata). That separation allows it to act as a lightweight,
/// portable container for cloning, serialization, or transport before
/// reconstructing a new `TransientHolon`.
#[derive(new, Debug, Clone, Serialize, Deserialize)]
pub struct HolonCloneModel {
    pub version: MapInteger,
    pub original_id: Option<LocalId>,
    pub properties: PropertyMap,
    pub relationships: Option<RelationshipMap>,
}

// =================================
//   HOLONBEHAVIOR IMPLEMENTATIONS
// =================================
impl ReadableHolonState for Holon {
    fn all_related_holons(&self) -> Result<RelationshipMap, HolonError> {
        match self {
            Holon::Transient(h) => h.all_related_holons(),
            Holon::Staged(h) => h.all_related_holons(),
            Holon::Saved(h) => {
                h.all_related_holons() // Will throw error, as not implemented, call must go through reference layer
            }
        }
    }

    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        match self {
            Holon::Transient(h) => h.essential_content(),
            Holon::Staged(h) => h.essential_content(),
            Holon::Saved(h) => h.essential_content(),
        }
    }

    fn holon_clone_model(&self) -> HolonCloneModel {
        match self {
            Holon::Transient(h) => h.holon_clone_model(),
            Holon::Staged(h) => h.holon_clone_model(),
            Holon::Saved(h) => h.holon_clone_model(),
        }
    }

    fn holon_id(&self) -> Result<HolonId, HolonError> {
        match self {
            Holon::Transient(h) => h.holon_id(),
            Holon::Staged(h) => h.holon_id(),
            Holon::Saved(h) => h.holon_id(),
        }
    }

    fn into_node_model(&self) -> HolonNodeModel {
        match self {
            Holon::Transient(h) => h.into_node_model(),
            Holon::Staged(h) => h.into_node_model(),
            Holon::Saved(h) => h.into_node_model(),
        }
    }

    fn key(&self) -> Result<Option<MapString>, HolonError> {
        match self {
            Holon::Transient(h) => h.key(),
            Holon::Staged(h) => h.key(),
            Holon::Saved(h) => h.key(),
        }
    }

    fn original_id(&self) -> Option<LocalId> {
        match self {
            Holon::Transient(h) => h.original_id(),
            Holon::Staged(h) => h.original_id(),
            Holon::Saved(h) => h.original_id(),
        }
    }

    fn property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        match self {
            Holon::Transient(h) => h.property_value(property_name),
            Holon::Staged(h) => h.property_value(property_name),
            Holon::Saved(h) => h.property_value(property_name),
        }
    }

    fn related_holons(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        match self {
            Holon::Transient(h) => h.related_holons(relationship_name),
            Holon::Staged(h) => h.related_holons(relationship_name),
            Holon::Saved(h) => h.related_holons(relationship_name),
        }
    }

    fn versioned_key(&self) -> Result<MapString, HolonError> {
        match self {
            Holon::Transient(h) => h.versioned_key(),
            Holon::Staged(h) => h.versioned_key(),
            Holon::Saved(h) => h.versioned_key(),
        }
    }

    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.is_accessible(access_type),
            Holon::Staged(h) => h.is_accessible(access_type),
            Holon::Saved(h) => h.is_accessible(access_type),
        }
    }

    fn summarize(&self) -> String {
        match self {
            Holon::Transient(h) => h.summarize(),
            Holon::Staged(h) => h.summarize(),
            Holon::Saved(h) => h.summarize(),
        }
    }
}

impl WriteableHolonState for Holon {
    fn add_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        match self {
            Holon::Transient(h) => {
                h.add_related_holons(context, relationship_name, holons)?;
                Ok(self)
            }
            Holon::Staged(h) => {
                h.add_related_holons(context, relationship_name, holons)?;
                Ok(self)
            }
            Holon::Saved(_) => {
                Err(HolonError::NotAccessible("Write".to_string(), "Saved".to_string()))
            }
        }
    }

    fn increment_version(&mut self) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.increment_version(),
            Holon::Staged(h) => h.increment_version(),
            Holon::Saved(_) => {
                Err(HolonError::NotAccessible("Write".to_string(), "Saved".to_string()))
            }
        }
    }

    fn mark_as_immutable(&mut self) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.mark_as_immutable(),
            Holon::Staged(h) => h.mark_as_immutable(),
            Holon::Saved(_) => Ok(()),
        }
    }

    fn remove_property_value(&mut self, property: &PropertyName) -> Result<&mut Self, HolonError> {
        match self {
            Holon::Transient(h) => {
                h.remove_property_value(property)?;
                Ok(self)
            }
            Holon::Staged(h) => {
                h.remove_property_value(property)?;
                Ok(self)
            }
            Holon::Saved(_) => {
                Err(HolonError::NotAccessible("Write".to_string(), "Saved".to_string()))
            }
        }
    }

    fn remove_related_holons(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        match self {
            Holon::Transient(h) => {
                h.remove_related_holons(context, relationship_name, holons)?;
                Ok(self)
            }
            Holon::Staged(h) => {
                h.remove_related_holons(context, relationship_name, holons)?;
                Ok(self)
            }
            Holon::Saved(_) => {
                Err(HolonError::NotAccessible("Write".to_string(), "Saved".to_string()))
            }
        }
    }

    fn update_original_id(&mut self, id: Option<LocalId>) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.update_original_id(id),
            Holon::Staged(h) => h.update_original_id(id),
            Holon::Saved(_) => {
                Err(HolonError::NotAccessible("Write".to_string(), "Saved".to_string()))
            }
        }
    }

    fn with_property_value(
        &mut self,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        match self {
            Holon::Transient(h) => {
                h.with_property_value(property, value)?;
                Ok(self)
            }
            Holon::Staged(h) => {
                h.with_property_value(property, value)?;
                Ok(self)
            }
            Holon::Saved(_) => {
                Err(HolonError::NotAccessible("Write".to_string(), "Saved".to_string()))
            }
        }
    }
}
