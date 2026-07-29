//! Storage-boundary types for version-aware holon node persistence (Storage SL2).
//!
//! These types are the vocabulary the guest storage layer speaks to everything above it.
//! They deliberately contain no Holochain concepts: the substrate's `Action`, `Record`, and
//! `original_action_address` stop at the persistence layer, which projects them into the
//! record-derived facts declared here.

mod types;

pub use types::*;
