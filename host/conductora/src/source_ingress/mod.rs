//! Native source selection and content snapshots; independent of MAP transactions.
pub(crate) mod commands;
mod discovery;
mod model;
pub(crate) mod picker;

pub use discovery::discover_sources;
pub use model::{SourceDiscovery, SourceFile, SourceIssue, SourceIssueKind, SourceSelection};
pub use picker::{
    pick_sources, source_picker_capabilities, SourcePickerCapabilities, SourceSelectionMode,
};

pub use commands::select_loader_sources;
