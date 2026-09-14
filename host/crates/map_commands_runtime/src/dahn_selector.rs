use base_types::MapString;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::descriptors::{equals_or_extends, walk_extends_chain};
use holons_core::reference_layer::{HolonReference, ReadableHolon};
use holons_core::Descriptor;
use map_commands_contract::{VisualizerKind, VisualizerSelection, VisualizerSelectionRequest};
use std::sync::Arc;
use type_names::DahnRelationshipTypeName;

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
pub fn select_canvas_visualizer(
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
/// request kinds supported by PR 3. It must not be read as a permanent
/// one-Visualizer-per-kind registry: future policy will choose among multiple
/// candidates using richer subjects, Slot context, and runtime information.
///
/// The subject is intentionally read at this boundary even though the current
/// policy does not score its descriptor. This reserves semantic selection for
/// Rust; TypeScript only instantiates the Visualizer Rust selected.
pub fn select_visualizer(
    context: &Arc<TransactionContext>,
    request: VisualizerSelectionRequest,
) -> Result<VisualizerSelection, HolonError> {
    if request.requested_kind == VisualizerKind::Canvas {
        let selected = select_canvas_visualizer(context, request.subject)?;
        return Ok(VisualizerSelection {
            selected: selected.visualizer,
            requested_kind: VisualizerKind::Canvas,
            alternatives_available: false,
        });
    }

    let selected = match request.requested_kind {
        VisualizerKind::Node => select_node_visualizer(context, request.subject)?,
        kind => {
            let visualizer_key = bootstrap_visualizer_key(kind)?;
            let _ = request.subject.holon_descriptor()?;
            HolonReference::Smart(
                context.lookup().get_saved_holon_by_key(&MapString::from(visualizer_key))?,
            )
        }
    };

    Ok(VisualizerSelection {
        selected,
        requested_kind: request.requested_kind,
        alternatives_available: false,
    })
}

/// Selects the nearest applicable Node Visualizer by walking the subject's
/// concrete descriptor lineage from leaf Type toward HolonType.
fn select_node_visualizer(
    context: &Arc<TransactionContext>,
    subject: HolonReference,
) -> Result<HolonReference, HolonError> {
    let node_visualizer_type = HolonReference::Smart(
        context.lookup().get_saved_holon_by_key(&MapString::from("NodeVisualizer.HolonType"))?,
    );
    let subject_type = subject.holon_descriptor()?.holon().clone();

    for type_descriptor in walk_extends_chain(&subject_type) {
        let type_descriptor = type_descriptor?;
        let members = type_descriptor
            .related_holons(DahnRelationshipTypeName::HasApplicableVisualizer)?
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
            .get_members()
            .clone();
        let mut candidates = Vec::new();
        for candidate in members {
            let candidate_type = candidate.holon_descriptor()?.holon().clone();
            if equals_or_extends(&candidate_type, &node_visualizer_type)? {
                candidates.push(candidate);
            }
        }

        match candidates.as_slice() {
            [] => continue,
            [candidate] => return Ok(candidate.clone()),
            _ => {
                return Err(HolonError::MultipleRelatedHolons {
                    relationship: DahnRelationshipTypeName::HasApplicableVisualizer
                        .as_relationship_name()
                        .to_string(),
                    descriptor: type_descriptor.summarize()?,
                    count: candidates.len(),
                });
            }
        }
    }

    Err(HolonError::NotImplemented(
        "No applicable Node Visualizer exists in the subject Type lineage".into(),
    ))
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
        | VisualizerKind::Properties
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
        assert!(bootstrap_visualizer_key(VisualizerKind::Properties).is_err());
        assert!(bootstrap_visualizer_key(VisualizerKind::Value).is_err());
        assert!(bootstrap_visualizer_key(VisualizerKind::Action).is_err());
    }
}
