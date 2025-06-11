use crate::core_shared_objects::HolonError;

use base_types::MapString;
use hdk::prelude::*;
use integrity_core_types::{HolonNode, LocalId, PropertyMap, PropertyName, PropertyValue};

use super::holon_utils::EssentialHolonContent;
use super::state::AccessType;
use super::{HolonBehavior, SavedHolon, StagedHolon, TransientHolon};

/// Enum representing the three Holon phases: `Transient`, `Staged`, and `Saved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Holon {
    Transient(TransientHolon),
    Staged(StagedHolon),
    Saved(SavedHolon),
}

// ==================================
//   ASSOCIATED METHODS (IMPL BLOCK)
// ==================================
impl Holon {
    /// Constructs a new `TransientHolon`.
    pub fn new_transient() -> Self {
        Holon::Transient(TransientHolon::new())
    }

    /// Gets inner TransientHolon object for Transient variant
    pub fn into_transient(self) -> Result<TransientHolon, HolonError> {
        match self {
            Holon::Transient(transient_holon) => Ok(transient_holon),
            _ => Err(HolonError::InvalidTransition("Holon variant must be Transient".to_string())),
        }
    }
}

// ================================
//   HOLONBEHAVIOR IMPLEMENTATION
// ================================
impl HolonBehavior for Holon {
    // ====================
    //    DATA ACCESSORS
    // ====================

    fn clone_holon(&self) -> Result<TransientHolon, HolonError> {
        match self {
            Holon::Transient(h) => h.clone_holon(),
            Holon::Staged(h) => h.clone_holon(),
            Holon::Saved(h) => h.clone_holon(),
        }
    }

    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        match self {
            Holon::Transient(h) => h.essential_content(),
            Holon::Staged(h) => h.essential_content(),
            Holon::Saved(h) => h.essential_content(),
        }
    }

    fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        match self {
            Holon::Transient(h) => h.get_key(),
            Holon::Staged(h) => h.get_key(),
            Holon::Saved(h) => h.get_key(),
        }
    }

    fn get_local_id(&self) -> Result<LocalId, HolonError> {
        match self {
            Holon::Transient(h) => h.get_local_id(),
            Holon::Staged(h) => h.get_local_id(),
            Holon::Saved(h) => h.get_local_id(),
        }
    }

    fn get_original_id(&self) -> Option<LocalId> {
        match self {
            Holon::Transient(h) => h.get_original_id(),
            Holon::Staged(h) => h.get_original_id(),
            Holon::Saved(h) => h.get_original_id(),
        }
    }

    fn get_property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        match self {
            Holon::Transient(h) => h.get_property_value(property_name),
            Holon::Staged(h) => h.get_property_value(property_name),
            Holon::Saved(h) => h.get_property_value(property_name),
        }
    }

    fn get_versioned_key(&self) -> Result<MapString, HolonError> {
        match self {
            Holon::Transient(h) => h.get_versioned_key(),
            Holon::Staged(h) => h.get_versioned_key(),
            Holon::Saved(h) => h.get_versioned_key(),
        }
    }

    fn into_node(&self) -> HolonNode {
        match self {
            Holon::Transient(h) => h.into_node(),
            Holon::Staged(h) => h.into_node(),
            Holon::Saved(h) => h.into_node(),
        }
    }

    // =================
    //     MUTATORS
    // =================

    fn update_original_id(&mut self, id: Option<LocalId>) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.update_original_id(id),
            Holon::Staged(h) => h.update_original_id(id),
            Holon::Saved(h) => h.update_original_id(id),
        }
    }

    fn update_property_map(&mut self, map: PropertyMap) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.update_property_map(map),
            Holon::Staged(h) => h.update_property_map(map),
            Holon::Saved(h) => h.update_property_map(map),
        }
    }

    fn increment_version(&mut self) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.increment_version(),
            Holon::Staged(h) => h.increment_version(),
            Holon::Saved(h) => h.increment_version(),
        }
    }

    // ======================
    //     ACCESS CONTROL
    // ======================

    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.is_accessible(access_type),
            Holon::Staged(h) => h.is_accessible(access_type),
            Holon::Saved(h) => h.is_accessible(access_type),
        }
    }

    // =====================
    //      DIAGNOSTICS
    // =====================

    fn debug_info(&self) -> String {
        match self {
            Holon::Transient(h) => h.debug_info(),
            Holon::Staged(h) => h.debug_info(),
            Holon::Saved(h) => h.debug_info(),
        }
    }

    // ===============
    //     HELPERS
    // ===============

    fn summarize(&self) -> String {
        match self {
            Holon::Transient(h) => h.summarize(),
            Holon::Staged(h) => h.summarize(),
            Holon::Saved(h) => h.summarize(),
        }
    }
}
