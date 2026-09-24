//! Host-native semantic selection for DAHN visualizers and home Dancers.
//!
//! This crate is deliberately stateless. Each selection resolves against the
//! caller's transaction-bound runtime context; it retains neither session
//! state nor a cache of semantic resources.

mod selection;

pub use selection::{
    select_bootstrap_canvas, select_collection_visualizer, select_home_dancer, select_visualizer,
    BootstrapCanvasSelection, HomeDancerRuntime, HomeDancerSelection, HomeDancerSelectionContext,
    RuntimeCanvasVisualizer,
};
