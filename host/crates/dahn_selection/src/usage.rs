use std::sync::Arc;

use base_types::MapString;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::descriptors::Descriptor;
use holons_core::{HolonReference, ReadableHolon, StagedReference, WritableHolon};
use map_commands_contract::VisualizerUsageSelection;

/// Holonic configuration anchor. Durable state is read through the bound reference.
#[derive(Debug, Clone)]
pub struct VisualizerUsage {
    holon: HolonReference,
}

/// Read relationship members after releasing the collection guard.
pub fn related(reference: &HolonReference, name: &str) -> Result<Vec<HolonReference>, HolonError> {
    Ok(reference
        .related_holons(name)?
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .clone())
}

/// Require exactly one relationship target, without choosing arbitrarily.
pub fn single(reference: &HolonReference, name: &str) -> Result<HolonReference, HolonError> {
    let members = related(reference, name)?;
    match members.as_slice() {
        [value] => Ok(value.clone()),
        _ => Err(HolonError::InvalidState(format!("Expected one {name}, found {}", members.len()))),
    }
}

/// Resolve a schema anchor through the transaction lookup surface.
pub fn anchor(context: &Arc<TransactionContext>, key: &str) -> Result<HolonReference, HolonError> {
    Ok(context.lookup().get_saved_holon_by_key(&MapString::from(key))?.into())
}

impl VisualizerUsage {
    /// Validate a usage's type and required identity relationships.
    pub fn from_holon(
        context: &Arc<TransactionContext>,
        holon: HolonReference,
    ) -> Result<Self, HolonError> {
        if single(&holon, "DescribedBy")? != anchor(context, "VisualizerUsage.HolonType")? {
            return Err(HolonError::InvalidParameter("Expected VisualizerUsage".into()));
        }
        single(&holon, "UsesVisualizer")?;
        single(&holon, "ForSubjectType")?;
        Ok(Self { holon })
    }

    /// Return the bound persisted configuration anchor.
    pub fn holon(&self) -> &HolonReference {
        &self.holon
    }
    /// Return the shared Visualizer whose personalization this usage records.
    pub fn visualizer(&self) -> Result<HolonReference, HolonError> {
        single(&self.holon, "UsesVisualizer")
    }
    /// Return the exact semantic descriptor used for configuration continuity.
    pub fn subject_type(&self) -> Result<HolonReference, HolonError> {
        single(&self.holon, "ForSubjectType")
    }
    /// Return retained successful-use associations, independent of remembered preference.
    pub fn selected_slots(&self) -> Result<Vec<HolonReference>, HolonError> {
        related(&self.holon, "SelectedForSlot")
    }
    /// Read property overrides; an empty set preserves default salience.
    pub fn property_salience_overrides(&self) -> Result<Vec<HolonReference>, HolonError> {
        related(&self.holon, "PropertySalienceOverrides")
    }
    /// Read relationship overrides; an empty set preserves default salience.
    pub fn relationship_salience_overrides(&self) -> Result<Vec<HolonReference>, HolonError> {
        related(&self.holon, "RelationshipSalienceOverrides")
    }
    /// Read action overrides; an empty set preserves default salience.
    pub fn action_salience_overrides(&self) -> Result<Vec<HolonReference>, HolonError> {
        related(&self.holon, "ActionSalienceOverrides")
    }
    /// Read action-group overrides; an empty set preserves Visualizer grouping.
    pub fn action_group_overrides(&self) -> Result<Vec<HolonReference>, HolonError> {
        related(&self.holon, "ActionGroupOverrides")
    }
}

/// Find persisted instances through the descriptor's committed inverse index.
pub fn instances(
    context: &Arc<TransactionContext>,
    type_key: &str,
) -> Result<Vec<HolonReference>, HolonError> {
    let descriptor = anchor(context, type_key)?;
    let members = related(&descriptor, "Instances")?;
    let mut found = Vec::new();
    for holon in members {
        if related(&holon, "DescribedBy")? == vec![descriptor.clone()] {
            found.push(holon);
        }
    }
    Ok(found)
}

/// Match the selected Visualizer's usages by exact subject descriptor identity.
/// Prior slot associations disambiguate multiple matching configurations.
pub fn find_usage(
    visualizer: &HolonReference,
    subject_type: &HolonReference,
    slot: &HolonReference,
) -> Result<Option<VisualizerUsage>, HolonError> {
    let mut matches = Vec::new();
    for holon in related(visualizer, "UsedByVisualizerUsage")? {
        if single(&holon, "ForSubjectType")? == *subject_type {
            matches.push(VisualizerUsage { holon });
        }
    }
    if matches.len() <= 1 {
        return Ok(matches.pop());
    }
    let mut associated = Vec::new();
    for usage in &matches {
        if usage.selected_slots()?.contains(slot) {
            associated.push(usage.clone());
        }
    }
    let applicable = if associated.is_empty() { &matches } else { &associated };
    match applicable.as_slice() {
        [] => Ok(None),
        [usage] => Ok(Some(usage.clone())),
        _ => Err(HolonError::InvalidState("Multiple applicable VisualizerUsages".into())),
    }
}

/// Stable local-space key for a descriptor-scoped semantic record.
pub fn record_key(
    prefix: &str,
    first: &HolonReference,
    second: &HolonReference,
) -> Result<MapString, HolonError> {
    let hex = |reference: &HolonReference| -> Result<String, HolonError> {
        Ok(reference.holon_id()?.local_id().0.iter().map(|byte| format!("{byte:02x}")).collect())
    };
    Ok(format!("{prefix}:{}:{}", hex(first)?, hex(second)?).into())
}

/// Stage a fully described usage. Missing override members are the four neutral sets.
pub fn initialize_usage(
    context: &Arc<TransactionContext>,
    visualizer: HolonReference,
    subject_type: HolonReference,
) -> Result<StagedReference, HolonError> {
    let usage_type = visualizer
        .holon_descriptor()?
        .resolve_available_relationship("UsedByVisualizerUsage")?
        .descriptor
        .target_type()?
        .holon()
        .clone();
    let mut usage = context.mutation().new_holon(Some(record_key(
        "visualizer-usage",
        &visualizer,
        &subject_type,
    )?))?;
    usage.add_related_holons("DescribedBy", vec![usage_type])?;
    usage.add_related_holons("UsesVisualizer", vec![visualizer])?;
    usage.add_related_holons("ForSubjectType", vec![subject_type])?;
    context.mutation().stage_new_holon(usage)
}

/// Project a committed usage into the caller's space capability, never the private transaction.
pub fn selection(
    context: &Arc<TransactionContext>,
    usage: HolonReference,
    initialized: bool,
    report_session: &str,
) -> Result<VisualizerUsageSelection, HolonError> {
    Ok(VisualizerUsageSelection {
        usage: HolonReference::smart_from_id(context.space_read_handle(), usage.holon_id()?),
        initialized,
        report_session: report_session.into(),
    })
}
