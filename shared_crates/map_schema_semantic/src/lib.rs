//! Shared Canonical Holon IR and derived semantic services for MAP schema tooling.
//!
//! The types in this crate sit between concrete source formats and loader-facing projections.
//! TDL, JSON imports, and future schema authoring formats should lower into this source-neutral
//! semantic model first, then build derived services such as symbol lookup or validation from it.
//! Keeping the model in a WASM-safe shared crate lets host tooling and hApp-reachable code agree on
//! descriptor vocabulary without depending on a host-only parser or a particular serialization
//! format.

pub mod diagnostics;
pub mod literal_value;
pub mod schema_index;
pub mod schema_ir;

pub use diagnostics::*;
pub use literal_value::*;
pub use schema_index::*;
pub use schema_ir::*;
