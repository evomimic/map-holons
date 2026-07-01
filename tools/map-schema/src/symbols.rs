//! Backward-compatible re-export of the canonical schema index module.
//!
//! V2-A2 makes the derived lookup layer explicit as [`crate::schema_index`]
//! while keeping existing `symbols` imports working during the migration.

pub use crate::schema_index::*;
