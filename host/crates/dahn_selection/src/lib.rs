//! Native DAHN selection facade and home Dancer orchestration.
//!
//! Portable semantic selection lives in holons_core for host and guest execution.
//! Selection has no global state. Each selection resolves against the caller's
//! bound context; a caller may supply composition-scoped semantic metadata.

pub use holons_core::dahn::{
    choose_visualizer, discover_visualizers, select_visualizer, select_visualizer_with_metadata,
    select_visualizer_with_subject_type, usage_subject_type, PreparedSelectionMetadata,
};
mod selection;
pub use selection::{
    select_bootstrap_canvas, select_collection_visualizer, select_home_dancer,
    BootstrapCanvasSelection, HomeDancerRuntime, HomeDancerSelection, HomeDancerSelectionContext,
    RuntimeCanvasVisualizer,
};

pub mod usage;
