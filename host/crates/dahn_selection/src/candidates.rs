use base_types::BaseValue;
use core_types::{HolonError, PropertyName, RelationshipName};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::descriptors::{equals_or_extends, walk_extends_chain, PropertyDescriptor};
use holons_core::{Descriptor, HolonReference, ReadableHolon};
use map_commands_contract::{
    VisualizerAssessment, VisualizerCandidate, VisualizerDiscovery, VisualizerKind,
    VisualizerOwner, VisualizerSelection, VisualizerSelectionRequest,
};
use std::sync::Arc;

fn members(reference: &HolonReference, name: &str) -> Result<Vec<HolonReference>, HolonError> {
    Ok(reference
        .related_holons(RelationshipName(name.into()))?
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .clone())
}

fn one(reference: &HolonReference, name: &str) -> Result<HolonReference, HolonError> {
    let values = members(reference, name)?;
    match values.as_slice() {
        [value] => Ok(value.clone()),
        [] => Err(HolonError::MissingRequiredRelationship {
            relationship: name.into(),
            descriptor: reference.summarize()?,
        }),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship: name.into(),
            descriptor: reference.summarize()?,
            count: many.len(),
        }),
    }
}

fn required_text(reference: &HolonReference, name: &str) -> Result<String, HolonError> {
    match reference.property_value(PropertyName(name.into()))? {
        Some(BaseValue::StringValue(value))
            if name != "VisualizerImplementationRuntime" && !value.0.is_empty() =>
        {
            Ok(value.0)
        }
        Some(BaseValue::EnumValue(value))
            if name == "VisualizerImplementationRuntime" && !value.0 .0.is_empty() =>
        {
            Ok(value.to_string())
        }
        _ => Err(HolonError::InvalidParameter(format!(
            "Missing or malformed {name} on Visualizer implementation"
        ))),
    }
}

/// Context resolution is shared; assessment does not choose among viable candidates.
struct AssessmentContext {
    accepted: Vec<HolonReference>,
    tokens: Vec<HolonReference>,
    start: HolonReference,
}

fn resolve(request: &VisualizerSelectionRequest) -> Result<AssessmentContext, HolonError> {
    let (owner, relation) = match &request.owner {
        VisualizerOwner::Visualizer(owner) => (owner, "HasSlot"),
        VisualizerOwner::Dancer(owner) => (owner, "HasExperienceVisualizerSlot"),
    };
    if !members(owner, relation)?.contains(&request.slot) {
        return Err(HolonError::InvalidParameter(
            "Requested slot does not belong to its stated composition owner".into(),
        ));
    }
    let accepted = members(&request.slot, "AcceptsVisualizerType")?;
    if accepted.is_empty() {
        return Err(HolonError::InvalidParameter("VisualizerSlot has no accepted types".into()));
    }
    let mds = one(&request.theme, "ForMetaDesignSystem")?;
    let tokens = members(&mds, "DefinesDesignToken")?;
    let start = match request.requested_kind {
        VisualizerKind::Property | VisualizerKind::Action => request.subject.clone(),
        VisualizerKind::Value => {
            PropertyDescriptor::from_holon(request.subject.clone()).value_type()?.holon().clone()
        }
        VisualizerKind::Node
        | VisualizerKind::PropertyMap
        | VisualizerKind::ActionBar
        | VisualizerKind::RootedNavigation
        | VisualizerKind::Structure => request.subject.holon_descriptor()?.holon().clone(),
        _ => {
            return Err(HolonError::InvalidParameter(
                "Use the dedicated Canvas or Collection selection contract".into(),
            ))
        }
    };
    Ok(AssessmentContext { accepted, tokens, start })
}

fn assess(
    context: &Arc<TransactionContext>,
    facts: &AssessmentContext,
    candidate: &HolonReference,
) -> Result<VisualizerAssessment, HolonError> {
    let candidate_type = candidate.holon_descriptor()?.holon().clone();
    let mut compatible = false;
    for accepted in &facts.accepted {
        if equals_or_extends(&candidate_type, accepted)? {
            compatible = true;
            break;
        }
    }
    if !compatible {
        return Ok(VisualizerAssessment::IncompatibleSlot);
    }
    if members(candidate, "ConsumesDesignToken")?.iter().any(|token| !facts.tokens.contains(token))
    {
        return Ok(VisualizerAssessment::IncompatibleTheme);
    }
    let implementations = members(candidate, "ImplementedBy")?;
    let implementation = match implementations.as_slice() {
        [] => return Ok(VisualizerAssessment::ImplementationUnavailable),
        [implementation] => implementation,
        many => {
            return Err(HolonError::MultipleRelatedHolons {
                relationship: "ImplementedBy".into(),
                descriptor: candidate.summarize()?,
                count: many.len(),
            })
        }
    };
    let runtime = required_text(implementation, "VisualizerImplementationRuntime")?;
    let format = required_text(implementation, "VisualizerModuleFormat")?;
    let _implementation_key = required_text(implementation, "VisualizerImplementationKey")?;
    let _entrypoint = required_text(implementation, "Entrypoint")?;
    let _digest = required_text(implementation, "VisualizerArtifactDigest")?;
    if runtime != "TypeScript"
        || format != "ESModule"
        || !context.visualizer_artifact_available(candidate)?
    {
        return Ok(VisualizerAssessment::ImplementationUnavailable);
    }
    Ok(VisualizerAssessment::Viable)
}

fn at_level(
    context: &Arc<TransactionContext>,
    facts: &AssessmentContext,
    descriptor: &HolonReference,
) -> Result<Vec<VisualizerCandidate>, HolonError> {
    let mut candidates: Vec<VisualizerCandidate> = Vec::new();
    for visualizer in members(descriptor, "HasApplicableVisualizer")? {
        if candidates.iter().any(|c| c.visualizer == visualizer) {
            continue;
        }
        let assessment = assess(context, facts, &visualizer)?;
        // Applicability declarations can offer several presentation kinds.
        // Only types accepted by this actual slot enter its choice population.
        if assessment == VisualizerAssessment::IncompatibleSlot {
            continue;
        }
        candidates.push(VisualizerCandidate {
            visualizer,
            declared_on: vec![descriptor.clone()],
            assessment,
        });
    }
    Ok(candidates)
}

fn boundary(descriptor: &HolonReference) -> Result<bool, HolonError> {
    Ok(descriptor.key()?.is_some_and(|key| key.0 == "HolonType.TypeDescriptor"))
}

/// The bootstrap winner policy is isolated from eligibility and traversal.
fn sole_viable(
    candidates: &[VisualizerCandidate],
    descriptor: &HolonReference,
) -> Result<Option<HolonReference>, HolonError> {
    let viable: Vec<_> =
        candidates.iter().filter(|c| c.assessment == VisualizerAssessment::Viable).collect();
    match viable.as_slice() {
        [] => Ok(None),
        [candidate] => Ok(Some(candidate.visualizer.clone())),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship: "HasApplicableVisualizer".into(),
            descriptor: descriptor.summarize()?,
            count: many.len(),
        }),
    }
}

/// Automatic selection reads no ancestor after nearest-level success or ambiguity.
pub fn select_visualizer(
    context: &Arc<TransactionContext>,
    request: VisualizerSelectionRequest,
) -> Result<VisualizerSelection, HolonError> {
    let facts = resolve(&request)?;
    for descriptor in walk_extends_chain(&facts.start) {
        let descriptor = descriptor?;
        if let Some(selected) = sole_viable(&at_level(context, &facts, &descriptor)?, &descriptor)?
        {
            return Ok(VisualizerSelection {
                selected,
                requested_kind: request.requested_kind,
                alternatives_available: false,
            });
        }
        if boundary(&descriptor)? {
            break;
        }
    }
    Err(HolonError::NotImplemented(
        "No viable Visualizer in the permitted descriptor lineage".into(),
    ))
}

/// Interactive discovery includes permitted ancestor alternatives without running winner policy.
pub fn discover_visualizers(
    context: &Arc<TransactionContext>,
    request: VisualizerSelectionRequest,
    current: Option<HolonReference>,
) -> Result<VisualizerDiscovery, HolonError> {
    let facts = resolve(&request)?;
    let mut candidates: Vec<VisualizerCandidate> = Vec::new();
    let mut ancestry = Vec::new();
    let mut stop_reason = map_commands_contract::DiscoveryStopReason::LineageExhausted;
    for descriptor in walk_extends_chain(&facts.start) {
        let descriptor = descriptor?;
        for candidate in at_level(context, &facts, &descriptor)? {
            if let Some(previous) =
                candidates.iter_mut().find(|c| c.visualizer == candidate.visualizer)
            {
                previous.declared_on.push(descriptor.clone());
            } else {
                candidates.push(candidate);
            }
        }
        let stop = boundary(&descriptor)?;
        ancestry.push(descriptor);
        if stop {
            stop_reason = map_commands_contract::DiscoveryStopReason::HolonTypeBoundary;
            break;
        }
    }
    let current_selection = current.map(|visualizer| {
        if let Some(candidate) = candidates.iter().find(|c| c.visualizer == visualizer) {
            VisualizerCandidate {
                visualizer,
                declared_on: candidate.declared_on.clone(),
                assessment: candidate.assessment,
            }
        } else {
            VisualizerCandidate {
                visualizer,
                declared_on: vec![],
                assessment: VisualizerAssessment::NoLongerApplicable,
            }
        }
    });
    Ok(VisualizerDiscovery { candidates, current_selection, ancestry, stop_reason, snapshot: None })
}

/// Explicit choice may bypass specificity/ambiguity, never current viability requirements.
pub fn choose_visualizer(
    context: &Arc<TransactionContext>,
    request: VisualizerSelectionRequest,
    candidate: HolonReference,
) -> Result<VisualizerSelection, HolonError> {
    let facts = resolve(&request)?;
    for descriptor in walk_extends_chain(&facts.start) {
        let descriptor = descriptor?;
        if members(&descriptor, "HasApplicableVisualizer")?.contains(&candidate) {
            let outcome = assess(context, &facts, &candidate)?;
            if outcome != VisualizerAssessment::Viable {
                return Err(HolonError::InvalidParameter(format!(
                    "Explicit Visualizer choice is not viable: {outcome:?}"
                )));
            }
            return Ok(VisualizerSelection {
                selected: candidate,
                requested_kind: request.requested_kind,
                alternatives_available: false,
            });
        }
        if boundary(&descriptor)? {
            break;
        }
    }
    Err(HolonError::InvalidParameter(
        "Explicit Visualizer is no longer applicable in the permitted ancestry".into(),
    ))
}

/// Resolve the semantic descriptor used for applicability and usage matching.
/// Value subjects use the selected value type, rather than the property's meta-type.
pub fn usage_subject_type(
    request: &VisualizerSelectionRequest,
) -> Result<HolonReference, HolonError> {
    Ok(resolve(request)?.start)
}
