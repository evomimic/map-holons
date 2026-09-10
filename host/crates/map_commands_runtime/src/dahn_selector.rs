use base_types::MapString;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::reference_layer::{HolonReference, ReadableHolon};
use map_commands_contract::{VisualizerKind, VisualizerSelection, VisualizerSelectionRequest};
use std::sync::Arc;

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
    let visualizer_key = bootstrap_visualizer_key(request.requested_kind)?;
    let _ = request.subject.holon_descriptor()?;
    let selected = context.lookup().get_saved_holon_by_key(&MapString::from(visualizer_key))?;

    Ok(VisualizerSelection {
        selected: HolonReference::Smart(selected),
        requested_kind: request.requested_kind,
        alternatives_available: false,
    })
}

/// Isolated deterministic bootstrap policy for currently bundled Visualizers.
///
/// The stable semantic key predates the `HolonInspectorVisualizer` design
/// name; it identifies the current least-specialized Node Visualizer, not the
/// only possible Node Visualizer. Properties, Value, and Action are DAHN-wide
/// kinds but have no PR 3 bootstrap selection yet, so requests for them fail
/// explicitly.
fn bootstrap_visualizer_key(kind: VisualizerKind) -> Result<&'static str, HolonError> {
    match kind {
        VisualizerKind::Canvas => Ok("SpaceNavigator.CanvasVisualizer"),
        VisualizerKind::Node => Ok("GenericHolonNodeVisualizer.NodeVisualizer"),
        VisualizerKind::Collection => Ok("TableCollectionVisualizer.CollectionVisualizer"),
        VisualizerKind::Properties | VisualizerKind::Value | VisualizerKind::Action => {
            Err(HolonError::NotImplemented(format!(
                "No deterministic DAHN bootstrap selection is configured for VisualizerKind::{kind:?}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::bootstrap_visualizer_key;
    use map_commands_contract::VisualizerKind;

    #[test]
    fn node_requests_use_the_holon_inspector_bootstrap_visualizer() {
        assert_eq!(
            bootstrap_visualizer_key(VisualizerKind::Node).expect("Node bootstrap visualizer"),
            "GenericHolonNodeVisualizer.NodeVisualizer"
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
        assert!(bootstrap_visualizer_key(VisualizerKind::Properties).is_err());
        assert!(bootstrap_visualizer_key(VisualizerKind::Value).is_err());
        assert!(bootstrap_visualizer_key(VisualizerKind::Action).is_err());
    }
}
