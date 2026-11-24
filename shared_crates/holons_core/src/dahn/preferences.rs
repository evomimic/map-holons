//! Authorized Agent-local preference population. Missing data is never an absent preference.
use super::candidates::{assess, one, AssessmentContext};
use super::contract::VisualizerAssessment;
use super::defaults::{optional, related};
use crate::agent_spaces::{sovereign_agent, SovereignAgent};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::{equals_or_extends, walk_extends_chain};
use crate::{HolonReference, ReadableHolon};
use core_types::HolonError;
use std::sync::Arc;
use type_names::{
    CoreRelationshipTypeName, DahnHolonTypeName, DahnRelationshipTypeName as R, ToRelationshipName,
};

#[derive(Clone, Debug)]
struct Preference {
    record: HolonReference,
    subject_type: HolonReference,
    slot: Option<HolonReference>,
    visualizer: HolonReference,
    usage: Option<HolonReference>,
}

/// Complete personal population for one admitted principal and resolution context.
/// The enclosing composition invalidates this product on a relevant semantic change.
#[derive(Clone, Debug, Default)]
pub struct PersonalPreferences {
    pub agent: Option<HolonReference>,
    records: Vec<Preference>,
}

#[derive(Clone, Debug)]
pub struct PersonalChoice {
    pub preference: HolonReference,
    pub visualizer: HolonReference,
    pub usage: Option<HolonReference>,
}
impl PersonalPreferences {
    /// Resolve authority from the adopted Space, never from a request-supplied Agent ID.
    pub fn for_context(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        context.get_space_holon()?.ok_or_else(|| {
            HolonError::InvalidState("Personal preference resolution requires an adopted Space; missing authority is not an absent preference".into())
        })?;
        let space = crate::agent_spaces::current_personal_space(context)?;
        match sovereign_agent(&space)? {
            SovereignAgent::Unclaimed => Ok(Self::default()),
            SovereignAgent::Ambiguous => {
                Err(HolonError::InvalidState("Ambiguous sovereign personal context".into()))
            }
            SovereignAgent::Agent(agent) => Self::load(context, agent),
        }
    }

    fn load(context: &Arc<TransactionContext>, agent: HolonReference) -> Result<Self, HolonError> {
        let mut result = Self { agent: Some(agent.clone()), records: Vec::new() };
        let sources = related(&agent, R::HasPersonalVisualizerPreference)?;
        if sources.is_empty() {
            return Ok(result);
        }
        let canonical = super::usage::anchor(
            context,
            &DahnHolonTypeName::VisualizerPreference.descriptor_key().0,
        )?;
        for record in sources {
            let described = record.related_holons(CoreRelationshipTypeName::DescribedBy)?;
            let described = described
                .read()
                .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
                .get_members()
                .clone();
            if described != vec![canonical.clone()] {
                return Err(HolonError::InvalidState("Personal preference requires the canonical general type; explicit conversion is required for an old Slot preference".into()));
            }
            if one(&record, &R::PreferenceForAgent.to_relationship_name().to_string())? != agent {
                return Err(HolonError::InvalidState(
                    "PreferenceForAgent disagrees with authorized inverse".into(),
                ));
            }
            let subject_type =
                one(&record, &R::PreferenceForSubjectType.to_relationship_name().to_string())?;
            let slot = optional(&record, R::PreferenceForSlot)?;
            if result.records.iter().any(|p| p.subject_type == subject_type && p.slot == slot) {
                return Err(HolonError::InvalidState(
                    "Conflicting personal preferences at one Agent/type/Slot scope".into(),
                ));
            }
            let visualizer =
                one(&record, &R::PreferredVisualizer.to_relationship_name().to_string())?;
            let usage = optional(&record, R::PreferredUsage)?;
            if let Some(usage) = &usage {
                if one(usage, &R::UsageForAgent.to_relationship_name().to_string())? != agent
                    || one(usage, &R::UsesVisualizer.to_relationship_name().to_string())?
                        != visualizer
                    || !equals_or_extends(
                        &subject_type,
                        &one(usage, &R::ForSubjectType.to_relationship_name().to_string())?,
                    )?
                {
                    return Err(HolonError::InvalidState("PreferredUsage has incompatible personal ownership, Visualizer, or configuration scope".into()));
                }
            }
            result.records.push(Preference { record, subject_type, slot, visualizer, usage });
        }
        Ok(result)
    }

    pub(crate) fn record_references(&self) -> Vec<HolonReference> {
        self.records.iter().map(|p| p.record.clone()).collect()
    }

    pub(crate) fn resolve(
        &self,
        context: &Arc<TransactionContext>,
        facts: &AssessmentContext,
    ) -> Result<Option<PersonalChoice>, HolonError> {
        if self.records.is_empty() {
            return Ok(None);
        }
        for local in [true, false] {
            if !self.records.iter().any(|p| {
                if local {
                    p.slot.as_ref() == Some(&facts.slot)
                } else {
                    p.slot.is_none()
                }
            }) {
                continue;
            }
            for subject_type in walk_extends_chain(&facts.start) {
                let subject_type = subject_type?;
                let candidate = self.records.iter().find(|p| {
                    p.subject_type == subject_type
                        && if local {
                            p.slot.as_ref() == Some(&facts.slot)
                        } else {
                            p.slot.is_none()
                        }
                });
                let Some(candidate) = candidate else {
                    continue;
                };
                let applicable = super::defaults::applicable(&facts.start, &candidate.visualizer)?;
                if applicable
                    && assess(context, facts, &candidate.visualizer)?
                        == VisualizerAssessment::Viable
                {
                    return Ok(Some(PersonalChoice {
                        preference: candidate.record.clone(),
                        visualizer: candidate.visualizer.clone(),
                        usage: candidate.usage.clone(),
                    }));
                }
                // A valid but ineligible preference yields for this Slot without mutation.
            }
        }
        Ok(None)
    }
}

/// Stage one explicit remembered choice. Ordinary Commit owns validation,
/// persistence, rejection and remediation; this operation never commits implicitly.
pub fn remember(
    context: &Arc<TransactionContext>,
    subject_type: HolonReference,
    slot: Option<HolonReference>,
    visualizer: HolonReference,
    usage: Option<HolonReference>,
) -> Result<HolonReference, HolonError> {
    use crate::WritableHolon;
    let population = PersonalPreferences::for_context(context)?;
    let agent = population.agent.ok_or_else(|| {
        HolonError::InvalidState(
            "Remembering a preference requires the authorized PersonAgent".into(),
        )
    })?;
    if !super::defaults::applicable(&subject_type, &visualizer)? {
        return Err(HolonError::InvalidParameter(
            "Preferred Visualizer is not applicable to the declared subject scope".into(),
        ));
    }
    if let Some(usage) = &usage {
        if one(usage, &R::UsageForAgent.to_relationship_name().to_string())? != agent
            || one(usage, &R::UsesVisualizer.to_relationship_name().to_string())? != visualizer
            || !equals_or_extends(
                &subject_type,
                &one(usage, &R::ForSubjectType.to_relationship_name().to_string())?,
            )?
        {
            return Err(HolonError::InvalidParameter(
                "Configuration does not belong to this Agent, Visualizer and subject scope".into(),
            ));
        }
    }
    let descriptor: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&DahnHolonTypeName::VisualizerPreference.descriptor_key())?
        .into();
    let mut staged = Vec::new();
    // Re-submission reuses the staged candidate, including a candidate rejected by Commit.
    for record in context.staged_references()? {
        let record: HolonReference = record.into();
        if super::usage::related(&record, "DescribedBy")? == vec![descriptor.clone()]
            && super::usage::related(&record, "PreferenceForAgent")? == vec![agent.clone()]
            && super::usage::related(&record, "PreferenceForSubjectType")?
                == vec![subject_type.clone()]
            && optional(&record, R::PreferenceForSlot)? == slot
        {
            staged.push(record);
        }
    }
    if staged.len() > 1 {
        return Err(HolonError::InvalidState("Conflicting staged personal preferences".into()));
    }
    let existing =
        population.records.iter().find(|p| p.subject_type == subject_type && p.slot == slot);
    let mut record: HolonReference = if let Some(staged) = staged.pop() {
        staged
    } else if let Some(existing) = existing {
        if existing.visualizer == visualizer && existing.usage == usage {
            return Ok(existing.record.clone());
        }
        context.mutation().stage_new_version_from_id(existing.record.holon_id()?)?.into()
    } else {
        let mut record = context.mutation().new_holon(None)?;
        record.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![descriptor])?;
        if let Some(space) = context.get_space_holon()? {
            record.add_related_holons(CoreRelationshipTypeName::OwnedBy, vec![space])?;
        }
        record.add_related_holons(R::PreferenceForAgent.to_relationship_name(), vec![agent])?;
        record.add_related_holons(
            R::PreferenceForSubjectType.to_relationship_name(),
            vec![subject_type],
        )?;
        if let Some(slot) = slot {
            record.add_related_holons(R::PreferenceForSlot.to_relationship_name(), vec![slot])?;
        }
        context.mutation().stage_new_holon(record)?.into()
    };
    for (relationship, values) in [
        (R::PreferredVisualizer, vec![visualizer]),
        (R::PreferredUsage, usage.into_iter().collect()),
    ] {
        let previous = related(&record, relationship)?;
        if previous != values {
            if !previous.is_empty() {
                record.remove_related_holons(relationship.to_relationship_name(), previous)?;
            }
            if !values.is_empty() {
                record.add_related_holons(relationship.to_relationship_name(), values)?;
            }
        }
    }
    Ok(record)
}

/// Explicit bounded conversion only. An absent old Slot is an error, never a
/// cross-cutting preference. Repeating a valid conversion returns the same choice.
pub fn convert_slot_preference(
    context: &Arc<TransactionContext>,
    old: &HolonReference,
) -> Result<HolonReference, HolonError> {
    use crate::WritableHolon;
    let old_type: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&DahnHolonTypeName::VisualizerSlotPreference.descriptor_key())?
        .into();
    if super::usage::related(old, "DescribedBy")? != vec![old_type] {
        return Err(HolonError::InvalidParameter(
            "Conversion requires an old Slot preference".into(),
        ));
    }
    let slot = one(old, "PreferenceForSlot")?;
    let subject = one(old, "PreferenceForSubjectType")?;
    let mut usage = one(old, "PreferredUsage")?;
    let visualizer = one(&usage, "UsesVisualizer")?;
    let population = PersonalPreferences::for_context(context)?;
    let agent = population.agent.as_ref().ok_or_else(|| {
        HolonError::InvalidState("Conversion requires the authorized PersonAgent".into())
    })?;
    let space = context
        .get_space_holon()?
        .ok_or_else(|| HolonError::InvalidState("Conversion requires the adopted Space".into()))?;
    if !same_lineage(context, &one(old, "OwnedBy")?, &space)? {
        return Err(HolonError::InvalidParameter(
            "Old preference must belong to the adopted Space".into(),
        ));
    }
    if let Some(existing) = population
        .records
        .iter()
        .find(|p| p.slot.as_ref() == Some(&slot) && p.subject_type == subject)
    {
        if existing.visualizer == visualizer
            && existing
                .usage
                .as_ref()
                .map(|u| same_lineage(context, u, &usage))
                .transpose()?
                .unwrap_or(false)
        {
            return Ok(existing.record.clone());
        }
        return Err(HolonError::InvalidState(
            "Conversion conflicts with an existing personal choice".into(),
        ));
    }
    let owner = optional(&usage, R::UsageForAgent)?;
    match owner {
        Some(owner) if same_lineage(context, &owner, agent)? => {}
        Some(_) => {
            return Err(HolonError::InvalidParameter(
                "Old configuration belongs to another Agent".into(),
            ))
        }
        None => {
            if !same_lineage(context, &one(&usage, "OwnedBy")?, &space)? {
                return Err(HolonError::InvalidParameter(
                    "Old preference and configuration must belong to the adopted Space".into(),
                ));
            }
            let mut staged = context.mutation().stage_new_version_from_id(usage.holon_id()?)?;
            staged
                .add_related_holons(R::UsageForAgent.to_relationship_name(), vec![agent.clone()])?;
            usage = staged.into();
        }
    }
    remember(context, subject, Some(slot), visualizer, Some(usage))
}

fn same_lineage(
    context: &Arc<TransactionContext>,
    left: &HolonReference,
    right: &HolonReference,
) -> Result<bool, HolonError> {
    Ok(crate::LineageIdentity::for_reference(left, context)?
        == crate::LineageIdentity::for_reference(right, context)?)
}
