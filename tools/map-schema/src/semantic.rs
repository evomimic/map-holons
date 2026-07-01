//! Backward-compatible re-export of the canonical schema IR module.
//!
//! V2-A1 extracts semantic ownership into [`crate::schema_ir`] while keeping
//! existing imports working until the remaining toolchain code is moved over.

pub use crate::schema_ir::*;
