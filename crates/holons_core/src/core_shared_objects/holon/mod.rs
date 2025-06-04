pub mod behavior;
pub mod holon_enum;
pub mod holon_utils;
pub mod saved;
pub mod staged;
pub mod state;
pub mod transient;


// Re-export core types for simplified usage
pub use behavior::HolonBehavior;
pub use transient::TransientHolon;
pub use staged::StagedHolon;
pub use saved::SavedHolon;
pub use holon_enum::Holon;