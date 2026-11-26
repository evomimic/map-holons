use std::sync::{Arc, Weak};

use super::contract::{VisualizerAssessment, VisualizerOwner, VisualizerSelectionRequest};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::HolonReference;
use core_types::HolonError;

use super::candidates::{assess, members, resolve, usage_subject_type, AssessmentContext};

struct CompatibilityMetadata {
    request: VisualizerSelectionRequest,
    facts: AssessmentContext,
}

/// Caller-owned metadata for one composition round in one transaction.
///
/// This index retains semantic facts, never a winner or Usage configuration.
/// The caller must replace it on semantic invalidation. Each actual subject
/// still supplies its descriptor and undergoes ancestry traversal and policy.
#[derive(Default)]
pub struct PreparedSelectionMetadata {
    owner: Option<Weak<TransactionContext>>,
    compatibility: Vec<CompatibilityMetadata>,
    declarations: Vec<(HolonReference, Vec<HolonReference>)>,
    assessments: Vec<(usize, HolonReference, VisualizerAssessment)>,
    personal: Option<super::PersonalPreferences>,
    defaults: Vec<(HolonReference, super::SlotDefaults)>,
    hits: usize,
    candidate_population_requests: usize,
}

impl PreparedSelectionMetadata {
    /// Populations actually observed in this round, including confirmed empty sets.
    pub(crate) fn observed_populations(
        &self,
    ) -> Vec<(HolonReference, String, Vec<HolonReference>)> {
        let mut observations = self
            .declarations
            .iter()
            .map(|(descriptor, members)| {
                (descriptor.clone(), "HasApplicableVisualizer".into(), members.clone())
            })
            .collect::<Vec<_>>();
        observations.extend(self.defaults.iter().map(|(slot, defaults)| {
            (slot.clone(), "HasDefaultDeclaration".into(), defaults.declaration_references())
        }));
        if let Some(personal) = &self.personal {
            if let Some(agent) = &personal.agent {
                observations.push((
                    agent.clone(),
                    "HasPersonalVisualizerPreference".into(),
                    personal.record_references(),
                ));
            }
        }
        observations
    }

    pub(crate) fn note_candidate_population_request(&mut self) {
        self.candidate_population_requests += 1;
    }

    pub(crate) fn population_requests(&self) -> (usize, usize) {
        (
            usize::from(self.personal.as_ref().is_some_and(|p| p.agent.is_some())),
            self.candidate_population_requests,
        )
    }

    /// Shared facts or assessments consumed without repeating their backend reads.
    pub fn hits(&self) -> usize {
        self.hits
    }

    pub(crate) fn resolve(
        &mut self,
        context: &Arc<TransactionContext>,
        request: &VisualizerSelectionRequest,
    ) -> Result<(AssessmentContext, Option<usize>), HolonError> {
        let same_owner = self
            .owner
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|owner| Arc::ptr_eq(&owner, context));
        if !same_owner {
            *self = Self { owner: Some(Arc::downgrade(context)), ..Self::default() };
        }
        if !saved_request(request) {
            return Ok((resolve(request)?, None));
        }
        if let Some(index) =
            self.compatibility.iter().position(|entry| same_compatibility(&entry.request, request))
        {
            let mut facts = self.compatibility[index].facts.clone();
            // A shared compatibility context does not substitute the first subject's type.
            facts.start = usage_subject_type(request)?;
            self.hits += 1;
            return Ok((facts, Some(index)));
        }
        let facts = resolve(request)?;
        let index = self.compatibility.len();
        self.compatibility
            .push(CompatibilityMetadata { request: request.clone(), facts: facts.clone() });
        Ok((facts, Some(index)))
    }

    pub(crate) fn personal(
        &mut self,
        context: &Arc<TransactionContext>,
    ) -> Result<super::PersonalPreferences, HolonError> {
        if let Some(personal) = &self.personal {
            return Ok(personal.clone());
        }
        let personal = super::PersonalPreferences::for_context(context)?;
        self.personal = Some(personal.clone());
        Ok(personal)
    }

    pub(crate) fn defaults(
        &mut self,
        slot: &HolonReference,
    ) -> Result<super::SlotDefaults, HolonError> {
        if let Some((_, defaults)) = self.defaults.iter().find(|(owner, _)| owner == slot) {
            self.hits += 1;
            return Ok(defaults.clone());
        }
        let defaults = super::SlotDefaults::load(slot)?;
        self.defaults.push((slot.clone(), defaults.clone()));
        Ok(defaults)
    }

    pub(crate) fn declarations(
        &mut self,
        descriptor: &HolonReference,
    ) -> Result<Vec<HolonReference>, HolonError> {
        if matches!(descriptor, HolonReference::Smart(_)) {
            if let Some((_, declarations)) =
                self.declarations.iter().find(|(source, _)| source == descriptor)
            {
                self.hits += 1;
                return Ok(declarations.clone());
            }
        }
        let declarations = members(descriptor, "HasApplicableVisualizer")?;
        if matches!(descriptor, HolonReference::Smart(_)) {
            self.declarations.push((descriptor.clone(), declarations.clone()));
        }
        Ok(declarations)
    }

    pub(crate) fn assess(
        &mut self,
        context: &Arc<TransactionContext>,
        compatibility: usize,
        facts: &AssessmentContext,
        candidate: &HolonReference,
    ) -> Result<VisualizerAssessment, HolonError> {
        if matches!(candidate, HolonReference::Smart(_)) {
            if let Some((_, _, assessment)) = self
                .assessments
                .iter()
                .find(|(scope, visualizer, _)| *scope == compatibility && visualizer == candidate)
            {
                self.hits += 1;
                return Ok(*assessment);
            }
        }
        let assessment = assess(context, facts, candidate)?;
        // Do not retain errors or unavailability: a later request can observe recovery.
        if matches!(candidate, HolonReference::Smart(_))
            && assessment != VisualizerAssessment::ImplementationUnavailable
        {
            self.assessments.push((compatibility, candidate.clone(), assessment));
        }
        Ok(assessment)
    }
}

fn saved_request(request: &VisualizerSelectionRequest) -> bool {
    let owner = match &request.owner {
        VisualizerOwner::Visualizer(owner) | VisualizerOwner::Dancer(owner) => owner,
    };
    request.value_holder.as_ref().is_none_or(|h| matches!(h, HolonReference::Smart(_)))
        && [&request.subject, &request.slot, &request.theme, owner]
            .iter()
            .all(|value| matches!(value, HolonReference::Smart(_)))
}

fn same_compatibility(a: &VisualizerSelectionRequest, b: &VisualizerSelectionRequest) -> bool {
    a.value_holder == b.value_holder
        && a.slot == b.slot
        && a.theme == b.theme
        && a.requested_kind == b.requested_kind
        && match (&a.owner, &b.owner) {
            (VisualizerOwner::Visualizer(a), VisualizerOwner::Visualizer(b))
            | (VisualizerOwner::Dancer(a), VisualizerOwner::Dancer(b)) => a == b,
            _ => false,
        }
}
