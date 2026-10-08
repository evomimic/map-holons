//! Host-native semantic selection for DAHN visualizers and home Dancers.
//!
//! This crate is deliberately stateless. Each selection resolves against the
//! caller's transaction-bound runtime context; it retains neither session
//! state nor a cache of semantic resources.

mod candidates;
mod selection;
pub use candidates::{choose_visualizer, discover_visualizers, select_visualizer};

pub use selection::{
    select_bootstrap_canvas, select_collection_visualizer, select_home_dancer,
    BootstrapCanvasSelection, HomeDancerRuntime, HomeDancerSelection, HomeDancerSelectionContext,
    RuntimeCanvasVisualizer,
};
