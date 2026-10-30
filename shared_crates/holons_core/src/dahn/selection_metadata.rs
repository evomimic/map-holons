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
    hits: usize,
}

impl PreparedSelectionMetadata {
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
                // Local artifact availability may change independently of semantic metadata.
                return Ok(
                    if *assessment == VisualizerAssessment::Viable
                        && !context.visualizer_artifact_available(candidate)?
                    {
                        VisualizerAssessment::ImplementationUnavailable
                    } else {
                        *assessment
                    },
                );
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
    [&request.subject, &request.slot, &request.theme, owner]
        .iter()
        .all(|value| matches!(value, HolonReference::Smart(_)))
}

fn same_compatibility(a: &VisualizerSelectionRequest, b: &VisualizerSelectionRequest) -> bool {
    a.slot == b.slot
        && a.theme == b.theme
        && a.requested_kind == b.requested_kind
        && match (&a.owner, &b.owner) {
            (VisualizerOwner::Visualizer(a), VisualizerOwner::Visualizer(b))
            | (VisualizerOwner::Dancer(a), VisualizerOwner::Dancer(b)) => a == b,
            _ => false,
        }
}
