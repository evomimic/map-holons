//! Host-side Holons Loader Client.
//!
//! This crate is responsible for:
//! - Validating loader import JSON files against the Holon Loader Schema.
//! - Parsing them into transient loader graph structures
//!   (`HolonLoadSet`, `HolonLoaderBundle`, `LoaderHolon`, etc.).
//! - Invoking the guest-side Holon Loader dance via the existing
//!   `HolonServiceApi::load_holons_internal` implementation.
//!
//! The main entrypoint for callers is
//! [`loader_client::load_holons_from_files`].
mod builder;
mod errors;
pub mod loader_client;
mod parser;
pub mod types;

// Public re-exports for the main entrypoint.
pub use loader_client::load_holons_from_files;

// Re-export key raw types + parsing diagnostics so tests and future
// tooling can use them without reaching into private modules.
pub use builder::{RawLoaderHolon, RawLoaderMeta, RawRelationshipEndpoints, RawRelationshipSpec};
pub use parser::{ImportFileParsingIssue, ImportFileParsingIssueKind, RawLoaderFileWithSlices};
pub use types::{ContentSet, FileData, BOOTSTRAP_IMPORT_SCHEMA_PATH};
