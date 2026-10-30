//! Read-only Usage resolution shared by guest execution and service test doubles.
use std::sync::Arc;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::{HolonReference, ReadableHolon, WritableHolon};
use core_types::HolonError;

/// Ephemeral resolution result. Configuration remains on the referenced Usage holon.
#[derive(Debug, Clone)]
pub struct VisualizerUsageResolution {
    pub usage: Option<HolonReference>,
    pub usage_type: Option<HolonReference>,
    pub default_type: HolonReference,
    /// Actual descriptor first, ending at the nearest direct applicability declaration.
    pub search_path: Vec<HolonReference>,
}

fn related(source: &HolonReference, name: &str) -> Result<Vec<HolonReference>, HolonError> {
    Ok(source
        .related_holons(name)?
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .clone())
}

fn one(source: &HolonReference, name: &str) -> Result<HolonReference, HolonError> {
    let members = related(source, name)?;
    match members.as_slice() {
        [value] => Ok(value.clone()),
        _ => Err(HolonError::InvalidState(format!("Expected one {name}, found {}", members.len()))),
    }
}

/// Resolve nearest committed configuration without creating a Usage or recording presentation.
/// Callers still authorize the selected Visualizer against the actual owner, Slot and Theme.
pub fn resolve_visualizer_usage(
    visualizer: &HolonReference,
    subject_type: &HolonReference,
    slot: &HolonReference,
) -> Result<VisualizerUsageResolution, HolonError> {
    let mut path = Vec::new();
    let mut current = subject_type.clone();
    loop {
        if path.contains(&current) {
            return Err(HolonError::InvalidState("Cycle in Usage subject Extends lineage".into()));
        }
        path.push(current.clone());
        if related(&current, "HasApplicableVisualizer")?.contains(visualizer) {
            break;
        }
        match related(&current, "Extends")?.as_slice() {
            [parent] => current = parent.clone(),
            [] => {
                return Err(HolonError::InvalidParameter(
                    "Visualizer has no applicability declaration in subject lineage".into(),
                ))
            }
            _ => {
                return Err(HolonError::InvalidState(
                    "Multiple Extends parents in Usage subject lineage".into(),
                ))
            }
        }
    }
    let default_type = current;
    // Fetch and classify the selected Visualizer's Usages once for the whole lineage.
    let usages = related(visualizer, "UsedByVisualizerUsage")?
        .into_iter()
        .map(|usage| Ok((one(&usage, "ForSubjectType")?, usage)))
        .collect::<Result<Vec<_>, HolonError>>()?;
    for descriptor in &path {
        let matches = usages
            .iter()
            .filter(|(bound, _)| bound == descriptor)
            .map(|(_, usage)| usage.clone())
            .collect::<Vec<_>>();
        let chosen = match matches.as_slice() {
            [] => continue,
            [usage] => usage.clone(),
            _ => {
                let associated = matches
                    .iter()
                    .map(|usage| Ok((usage, related(usage, "SelectedForSlot")?.contains(slot))))
                    .collect::<Result<Vec<_>, HolonError>>()?
                    .into_iter()
                    .filter(|(_, associated)| *associated)
                    .map(|(usage, _)| usage.clone())
                    .collect::<Vec<_>>();
                match associated.as_slice() {
                    [usage] => usage.clone(),
                    _ => {
                        return Err(HolonError::InvalidState(
                            "Multiple applicable VisualizerUsages".into(),
                        ))
                    }
                }
            }
        };
        if one(&chosen, "UsesVisualizer")? != *visualizer {
            return Err(HolonError::InvalidState(
                "Usage inverse membership disagrees with UsesVisualizer".into(),
            ));
        }
        return Ok(VisualizerUsageResolution {
            usage: Some(chosen),
            usage_type: Some(descriptor.clone()),
            default_type,
            search_path: path,
        });
    }
    Ok(VisualizerUsageResolution { usage: None, usage_type: None, default_type, search_path: path })
}

impl VisualizerUsageResolution {
    /// Project into a typed transient body for canonical and internal guest Dance responses.
    pub fn into_projection(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonReference, HolonError> {
        let descriptor = context
            .lookup()
            .get_saved_holon_by_key(&"VisualizerUsageResolution.Projection".into())?;
        let mut body: HolonReference =
            context.mutation().new_holon(Some("visualizer-usage-resolution".into()))?.into();
        body.add_related_holons("DescribedBy", vec![descriptor.into()])?;
        body.add_related_holons("DefaultUsageType", vec![self.default_type])?;
        body.add_related_holons("UsageSearchPath", self.search_path)?;
        if let Some(usage) = self.usage {
            body.add_related_holons("ResolvedUsage", vec![usage])?;
        }
        if let Some(descriptor) = self.usage_type {
            body.add_related_holons("ResolvedUsageType", vec![descriptor])?;
        }
        Ok(body)
    }

    /// Read a guest projection without loading any saved configuration on the client.
    pub fn from_projection(body: &HolonReference) -> Result<Self, HolonError> {
        let optional = |name| -> Result<Option<HolonReference>, HolonError> {
            match related(body, name)?.as_slice() {
                [] => Ok(None),
                [reference] => Ok(Some(reference.clone())),
                _ => Err(HolonError::InvalidState(format!("Multiple {name} in Usage resolution"))),
            }
        };
        let result = Self {
            usage: optional("ResolvedUsage")?,
            usage_type: optional("ResolvedUsageType")?,
            default_type: one(body, "DefaultUsageType")?,
            search_path: related(body, "UsageSearchPath")?,
        };
        if result.usage.is_some() != result.usage_type.is_some()
            || result.search_path.last() != Some(&result.default_type)
            || result.usage_type.as_ref().is_some_and(|scope| !result.search_path.contains(scope))
        {
            return Err(HolonError::InvalidState(
                "Inconsistent Usage resolution projection".into(),
            ));
        }
        Ok(result)
    }
}
