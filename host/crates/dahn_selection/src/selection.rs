use base_types::MapString;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::descriptors::{equals_or_extends, walk_extends_chain, PropertyDescriptor};
use holons_core::reference_layer::{HolonReference, ReadableHolon};
use holons_core::Descriptor;
use map_commands_contract::{VisualizerKind, VisualizerSelection, VisualizerSelectionRequest};
use std::sync::Arc;
use type_names::{DahnRelationshipTypeName, DancerRelationshipTypeName};

/// Bound runtime realization of the Canvas selected for one application
/// session. The Canvas remains a semantic holon; this wrapper carries the
/// separately selected Canvas Visualizer without making either a TypeScript
/// registry identity.
pub struct RuntimeCanvasVisualizer {
    pub canvas: HolonReference,
    pub visualizer: HolonReference,
}

/// Rust-authorized bootstrap outcome for the Theme-to-Canvas launch path.
///
/// The policy has exactly one bootstrap candidate for each selection. Missing
/// resources fail through normal lookup rather than being substituted by a
/// caller or the TypeScript runtime.
pub struct BootstrapCanvasSelection {
    pub theme: HolonReference,
    pub canvas_visualizer: RuntimeCanvasVisualizer,
}

/// Runtime-only launch circumstances consulted by home-Dancer selection.
///
/// This is deliberately not an IPC wire type or persisted holon. Rust owns
/// selection policy; the optional person reference leaves room for a later
/// HolonSpace-scoped home preference without inventing one in this slice.
pub struct HomeDancerSelectionContext {
    pub active_holon_space: HolonReference,
    pub selected_theme: HolonReference,
    pub selected_meta_design_system: HolonReference,
    pub runtime: HomeDancerRuntime,
    pub person: Option<HolonReference>,
}

/// The local launch runtime available to the initial selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeDancerRuntime {
    Local,
}

/// Rust-selected home Dancer and the two concrete visualizers in its initial
/// experience composition. The Dancer owns the Structure slot; the Structure
/// visualizer owns the Node slot.
pub struct HomeDancerSelection {
    pub dancer: HolonReference,
    pub rooted_navigation_visualizer: HolonReference,
    pub root_node_visualizer: HolonReference,
}

/// Resolves the active HolonSpace's afforded Dancer candidates.
///
/// A missing declaration is a deliberate empty-home result. A non-empty
/// declaration with no Dancer that can realize its declared RootedNavigation
/// experience is an error; callers must not substitute a named visualizer.
pub fn select_home_dancer(
    context: &Arc<TransactionContext>,
    selection_context: HomeDancerSelectionContext,
) -> Result<Option<HomeDancerSelection>, HolonError> {
    // Read each context member at the Rust boundary. The first policy has no
    // person-specific preference or runtime alternatives, but it deliberately
    // receives the complete context that later policy will evaluate.
    let _ = selection_context.active_holon_space.holon_descriptor()?;
    let _ = selection_context.selected_theme.holon_descriptor()?;
    let _ = selection_context.selected_meta_design_system.holon_descriptor()?;
    if let Some(person) = &selection_context.person {
        let _ = person.holon_descriptor()?;
    }
    match selection_context.runtime {
        HomeDancerRuntime::Local => {}
    }

    let candidates = selection_context
        .active_holon_space
        .related_holons(DancerRelationshipTypeName::AffordsDancer)?
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
        .get_members()
        .clone();
    if candidates.is_empty() {
        return Ok(None);
    }

    let dancer_type = HolonReference::Smart(
        context.lookup().get_saved_holon_by_key(&MapString::from("Dancer.HolonType"))?,
    );
    let mut compatible = Vec::new();
    for candidate in candidates {
        let candidate_type = candidate.holon_descriptor()?.holon().clone();
        if !equals_or_extends(&candidate_type, &dancer_type)? {
            continue;
        }
        let dancer_slot = require_single_related(
            &candidate,
            DancerRelationshipTypeName::HasExperienceVisualizerSlot,
            "Dancer experience VisualizerSlot",
        )?;
        let subject_type = selection_context.active_holon_space.holon_descriptor()?.holon().clone();
        match select_for_slot(subject_type.clone(), &dancer_slot) {
            Ok(rooted_navigation_visualizer) => {
                let path_slot = require_single_related(
                    &rooted_navigation_visualizer,
                    DahnRelationshipTypeName::HasSlot,
                    "RootedNavigation NodeVisualizerSlot",
                )?;
                let root_node_visualizer = select_for_slot(subject_type, &path_slot)?;
                compatible.push(HomeDancerSelection {
                    dancer: candidate,
                    rooted_navigation_visualizer,
                    root_node_visualizer,
                });
            }
            Err(HolonError::NotImplemented(_)) => {}
            Err(error) => return Err(error),
        }
    }

    match compatible.len() {
        0 => Err(HolonError::NotImplemented(
            "No declared home Dancer has a selectable RootedNavigation experience".into(),
        )),
        1 => Ok(compatible.pop()),
        count => Err(HolonError::MultipleRelatedHolons {
            relationship: DancerRelationshipTypeName::AffordsDancer
                .as_relationship_name()
                .to_string(),
            descriptor: selection_context.active_holon_space.summarize()?,
            count,
        }),
    }
}

pub fn select_bootstrap_canvas(
    context: &Arc<TransactionContext>,
) -> Result<BootstrapCanvasSelection, HolonError> {
    let theme = HolonReference::Smart(
        context.lookup().get_saved_holon_by_key(&MapString::from("MAP.BootstrapTheme"))?,
    );
    let canvas = HolonReference::Smart(
        context.lookup().get_saved_holon_by_key(&MapString::from("MAP.BootstrapCanvas"))?,
    );
    let canvas_visualizer = select_canvas_visualizer(context, canvas)?;
    Ok(BootstrapCanvasSelection { theme, canvas_visualizer })
}

/// Selects the generic Canvas Visualizer for an already selected Canvas holon.
///
/// This is intentionally a dedicated service seam rather than a fallback in a
/// caller. The bootstrap policy currently has one compatible Canvas
/// Visualizer; a missing resource remains an error from the lookup.
fn select_canvas_visualizer(
    context: &Arc<TransactionContext>,
    canvas: HolonReference,
) -> Result<RuntimeCanvasVisualizer, HolonError> {
    let _ = canvas.holon_descriptor()?;
    let visualizer_key = bootstrap_visualizer_key(VisualizerKind::Canvas)?;
    let visualizer = context.lookup().get_saved_holon_by_key(&MapString::from(visualizer_key))?;

    Ok(RuntimeCanvasVisualizer { canvas, visualizer: HolonReference::Smart(visualizer) })
}

/// Resolves a visualization request through the DAHN Selector Function.
///
/// The current implementation is a deterministic bootstrap policy for the
/// request kinds supported by the initial DAHN slice. It must not be read as a permanent
/// one-Visualizer-per-kind registry: future policy will choose among multiple
/// candidates using richer subjects, Slot context, and runtime information.
///
/// The subject is intentionally read at this boundary even though the current
/// policy does not score its descriptor. This reserves semantic selection for
/// Rust; TypeScript only instantiates the Visualizer Rust selected.
pub fn select_visualizer(
    _context: &Arc<TransactionContext>,
    request: VisualizerSelectionRequest,
) -> Result<VisualizerSelection, HolonError> {
    let VisualizerSelectionRequest { subject, requested_kind, parent_visualizer, slot } = request;
    if let Some(parent) = parent_visualizer {
        let slots = parent.related_holons(DahnRelationshipTypeName::HasSlot)?;
        if !slots
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .get_members()
            .iter()
            .any(|owned| owned.reference_id_string() == slot.reference_id_string())
        {
            return Err(HolonError::InvalidParameter(
                "Requested slot does not belong to parent Visualizer".into(),
            ));
        }
    }
    let descriptor = match requested_kind {
        VisualizerKind::Property => subject,
        VisualizerKind::Value => {
            PropertyDescriptor::from_holon(subject).value_type()?.holon().clone()
        }
        VisualizerKind::Node
        | VisualizerKind::PropertyMap
        | VisualizerKind::Action
        | VisualizerKind::RootedNavigation => subject.holon_descriptor()?.holon().clone(),
        _ => {
            return Err(HolonError::InvalidParameter(
                "Use the dedicated Canvas or Collection selection contract".into(),
            ))
        }
    };
    let selected = select_for_slot(descriptor, &slot)?;
    Ok(VisualizerSelection { selected, requested_kind, alternatives_available: false })
}

/// Select at the nearest compatible level, never crossing the nearest local TKD.
fn select_for_slot(
    subject_type: HolonReference,
    slot: &HolonReference,
) -> Result<HolonReference, HolonError> {
    let accepted = slot
        .related_holons(DahnRelationshipTypeName::AcceptsVisualizerType)?
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .clone();
    if accepted.is_empty() {
        return Err(HolonError::InvalidParameter("VisualizerSlot has no accepted types".into()));
    }
    for descriptor in walk_extends_chain(&subject_type) {
        let descriptor = descriptor?;
        let members = descriptor
            .related_holons(DahnRelationshipTypeName::HasApplicableVisualizer)?
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .get_members()
            .clone();
        let mut candidates = Vec::new();
        for candidate in members {
            let candidate_type = candidate.holon_descriptor()?.holon().clone();
            for accepted_type in &accepted {
                if equals_or_extends(&candidate_type, accepted_type)? {
                    if !candidates.iter().any(|c: &HolonReference| {
                        c.reference_id_string() == candidate.reference_id_string()
                    }) {
                        candidates.push(candidate.clone());
                    }
                    break;
                }
            }
        }
        match candidates.as_slice() {
            [candidate] => return Ok(candidate.clone()),
            [] => {}
            many => {
                return Err(HolonError::MultipleRelatedHolons {
                    relationship: "HasApplicableVisualizer".into(),
                    descriptor: descriptor.summarize()?,
                    count: many.len(),
                })
            }
        }
        if holons_core::descriptors::HolonDescriptor::from_holon(descriptor)
            .header()
            .defines_instance_type_kind()?
        {
            return Err(HolonError::NotImplemented(
                "No slot-compatible Visualizer at or below the subject TypeKind definer".into(),
            ));
        }
    }
    Err(HolonError::NotImplemented(
        "No applicable Visualizer before descriptor lineage exhaustion".into(),
    ))
}

fn require_single_related<T: type_names::ToRelationshipName>(
    holon: &HolonReference,
    relationship: T,
    description: &str,
) -> Result<HolonReference, HolonError> {
    let members = holon
        .related_holons(relationship)?
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
        .get_members()
        .clone();
    match members.as_slice() {
        [member] => Ok(member.clone()),
        [] => Err(HolonError::NotImplemented(format!("Missing {description}"))),
        _ => Err(HolonError::NotImplemented(format!("Ambiguous {description}"))),
    }
}

/// Isolated deterministic bootstrap policy for currently bundled Visualizers.
///
/// Only Canvas and Collection remain bootstrap-keyed. Node selection is
/// applicability-driven; Properties, Value, and Action have no bootstrap
/// selection and therefore fail explicitly.
fn bootstrap_visualizer_key(kind: VisualizerKind) -> Result<&'static str, HolonError> {
    match kind {
        // The bootstrap Canvas is a DAHN-wide resource. It is deliberately
        // not owned by, or named after, the Space Navigator Dancer.
        VisualizerKind::Canvas => Ok("MAP.BootstrapCanvasVisualizer"),
        VisualizerKind::Collection => Ok("TableCollectionVisualizer.CollectionVisualizer"),
        VisualizerKind::Node
        | VisualizerKind::RootedNavigation
        | VisualizerKind::PropertyMap
        | VisualizerKind::Property
        | VisualizerKind::Value
        | VisualizerKind::Action => Err(HolonError::NotImplemented(format!(
            "No deterministic DAHN bootstrap selection is configured for VisualizerKind::{kind:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::bootstrap_visualizer_key;
    use map_commands_contract::VisualizerKind;

    #[test]
    fn canvas_requests_use_the_generic_bootstrap_canvas_visualizer() {
        assert_eq!(
            bootstrap_visualizer_key(VisualizerKind::Canvas).expect("Canvas bootstrap visualizer"),
            "MAP.BootstrapCanvasVisualizer"
        );
    }

    #[test]
    fn bootstrap_selection_is_deterministic() {
        assert_eq!(
            bootstrap_visualizer_key(VisualizerKind::Collection)
                .expect("Collection bootstrap visualizer"),
            bootstrap_visualizer_key(VisualizerKind::Collection)
                .expect("Collection bootstrap visualizer")
        );
    }

    #[test]
    fn unsupported_kinds_fail_without_selecting_an_unrelated_visualizer() {
        assert!(bootstrap_visualizer_key(VisualizerKind::Node).is_err());
        assert!(bootstrap_visualizer_key(VisualizerKind::PropertyMap).is_err());
        assert!(bootstrap_visualizer_key(VisualizerKind::Property).is_err());
        assert!(bootstrap_visualizer_key(VisualizerKind::Value).is_err());
        assert!(bootstrap_visualizer_key(VisualizerKind::Action).is_err());
    }
}

/// Selects a collection implementation without depending on its producer.
pub fn select_collection_visualizer(
    context: &Arc<TransactionContext>,
    collection: map_commands_contract::DescribedHolonCollection,
    parent: HolonReference,
    slot: HolonReference,
) -> Result<VisualizerSelection, HolonError> {
    let _ = holons_core::descriptors::HolonDescriptor::from_holon(collection.element_type.clone())
        .instance_properties()?;
    let slots = parent
        .related_holons(DahnRelationshipTypeName::HasSlot)?
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
        .get_members()
        .clone();
    if !slots.iter().any(|candidate| candidate.reference_id_string() == slot.reference_id_string())
    {
        return Err(HolonError::InvalidParameter(
            "Collection slot does not belong to its parent".into(),
        ));
    }
    let selected = HolonReference::Smart(context.lookup().get_saved_holon_by_key(
        &MapString::from(bootstrap_visualizer_key(VisualizerKind::Collection)?),
    )?);
    let child_type = selected.holon_descriptor()?.holon().clone();
    let accepted = slot
        .related_holons(DahnRelationshipTypeName::AcceptsVisualizerType)?
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
        .get_members()
        .clone();
    for accepted_type in accepted {
        if equals_or_extends(&child_type, &accepted_type)? {
            return Ok(VisualizerSelection {
                selected,
                requested_kind: VisualizerKind::Collection,
                alternatives_available: false,
            });
        }
    }
    Err(HolonError::InvalidParameter(
        "Selected Collection Visualizer is incompatible with the destination slot".into(),
    ))
}
