//! Portable DAHN selection and type-owned composition Slots.
mod candidates;
pub mod composition;
pub mod contract;
mod operators;
mod selection_metadata;
pub mod usage;
pub use candidates::{
    choose_visualizer, discover_visualizers, select_visualizer, select_visualizer_with_metadata,
    select_visualizer_with_subject_type, usage_subject_type,
};
pub use selection_metadata::PreparedSelectionMetadata;

mod slots;
pub use operators::fulfills_operators;
pub use slots::composition_slots;

mod composition_contract;
pub use composition_contract::{CompositionActivation, CompositionSubject};

pub(crate) mod defaults;
pub use defaults::SlotDefaults;

pub mod preferences;
pub use preferences::{PersonalChoice, PersonalPreferences};
