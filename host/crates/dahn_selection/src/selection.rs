use base_types::MapString;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::descriptors::equals_or_extends;
use holons_core::reference_layer::{HolonReference, ReadableHolon};
use holons_core::Descriptor;
use map_commands_contract::{
    VisualizerKind, VisualizerOwner, VisualizerSelection, VisualizerSelectionRequest,
};
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
        let navigation_type =
            HolonReference::Smart(context.lookup().get_saved_holon_by_key(&MapString::from(
                "RootedNavigationVisualizer.HolonType",
            ))?);
        let slots =
            candidate.related_holons(DancerRelationshipTypeName::HasExperienceVisualizerSlot)?;
        let slots = slots
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .get_members()
            .clone();
        let mut navigation_slots = Vec::new();
        for slot in slots {
            let accepted = slot.related_holons(DahnRelationshipTypeName::AcceptsVisualizerType)?;
            let accepted = accepted
                .read()
                .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
                .get_members()
                .clone();
            for accepted_type in accepted {
                if equals_or_extends(&accepted_type, &navigation_type)? {
                    navigation_slots.push(slot.clone());
                    break;
                }
            }
        }
        if navigation_slots.len() != 1 {
            return Err(HolonError::InvalidParameter(format!(
                "Dancer must declare one RootedNavigation slot, found {}",
                navigation_slots.len()
            )));
        }
        let dancer_slot = navigation_slots.remove(0);
        match crate::select_visualizer(
            context,
            VisualizerSelectionRequest {
                subject: selection_context.active_holon_space.clone(),
                requested_kind: VisualizerKind::RootedNavigation,
                owner: VisualizerOwner::Dancer(candidate.clone()),
                slot: dancer_slot,
                theme: selection_context.selected_theme.clone(),
            },
        ) {
            Ok(selection) => {
                let rooted_navigation_visualizer = selection.selected;
                let slots = holons_core::dahn::composition_slots(&rooted_navigation_visualizer)?;
                let path_slot = match slots.as_slice() {
                    [slot] => slot.clone(),
                    _ => {
                        return Err(HolonError::InvalidParameter(
                            "Expected one RootedNavigation Node Slot".into(),
                        ))
                    }
                };
                let root_node_visualizer = crate::select_visualizer(
                    context,
                    VisualizerSelectionRequest {
                        subject: selection_context.active_holon_space.clone(),
                        requested_kind: VisualizerKind::Node,
                        owner: VisualizerOwner::Visualizer(rooted_navigation_visualizer.clone()),
                        slot: path_slot,
                        theme: selection_context.selected_theme.clone(),
                    },
                )?
                .selected;
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
        context.lookup().get_saved_holon_by_key(&MapString::from("Demo1.DeepOceanTheme"))?,
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
        VisualizerKind::TitleBar
        | VisualizerKind::Node
        | VisualizerKind::Structure
        | VisualizerKind::RootedNavigation
        | VisualizerKind::PropertyMap
        | VisualizerKind::Property
        | VisualizerKind::Value
        | VisualizerKind::ActionBar
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
    let _ = holons_core::descriptors::EffectiveHolonDescriptor::acquire(
        collection.element_type.clone(),
    )?
    .instance_properties()?;
    let slots = holons_core::dahn::composition_slots(&parent)?;
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
                usage: None,
            });
        }
    }
    Err(HolonError::InvalidParameter(
        "Selected Collection Visualizer is incompatible with the destination slot".into(),
    ))
}
