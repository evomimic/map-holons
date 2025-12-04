//! Top-level entrypoint for the host-side Holons Loader Client.
//!
//! This module is called from the native receptor layer
//! (`ReceptorFactory::load_holons(...)`) and orchestrates:
//!
//! 1. Parsing & validation of one or more loader import files into a
//!    single `HolonLoadSet` via [`crate::parser`] and [`crate::builder`].
//! 2. Invoking the guest-side Holon Loader dance by delegating to the
//!    reference-layer helper `holons_core::reference_layer::load_holons`,
//!    which in turn calls `HolonServiceApi::load_holons_internal` on the
//!    client holon service.
//! 3. Returning a `TransientReference` to the resulting `HolonLoadResponse`
//!    (or a `HolonError` if validation/parsing fails).
//!
//! The TypeScript side remains stateless and will later use lightweight
//! dances/commands to navigate the response graph via references.

use std::path::PathBuf;
use std::sync::Arc;

use base_types::MapString;
use core_types::HolonError;
use holons_core::reference_layer::{load_holons, HolonsContextBehavior, TransientReference};
use holons_core::HolonReference;

use crate::errors::map_parsing_issues_to_holon_error;
use crate::parser::parse_files_into_load_set;

/// Primary entry point for the host-side Holon Loader.
///
/// This function is intended to be called from `ReceptorFactory::load_holons(...)`.
///
/// High-level behavior:
/// - If no input files are provided, returns `HolonError::InvalidParameter`.
/// - Calls [`parse_files_into_load_set`] to:
///     - validate each file against the loader JSON Schema, and
///     - build a single `HolonLoadSet` holon with per-file bundles.
/// - If parsing or validation fails for any file, aggregates the resulting
///   `ImportFileParsingIssue`s into a single `HolonError` via
///   [`map_parsing_issues_to_holon_error`] and returns early.
/// - If parsing succeeds:
///     - Ensures the returned `HolonReference` is a transient reference
///       to the `HolonLoadSet`.
///     - Invokes the guest-side loader dance via
///       [`holons_core::reference_layer::load_holons`], passing the
///       `HolonLoadSet` transient reference.
///     - Returns a `TransientReference` to the resulting `HolonLoadResponse`
///       holon on success.
///
/// Notes:
/// - This API is `async` to fit naturally into the receptor/command
///   plumbing, but currently delegates only to synchronous Rust code.
///   The underlying dance execution is already bridged from async → sync
///   inside the `HolonServiceApi` implementation.
pub async fn load_holons_from_files(
    context: Arc<dyn HolonsContextBehavior>,
    import_file_paths: &[PathBuf],
) -> Result<TransientReference, HolonError> {
    // Guard against an empty file list; this is almost certainly a caller bug.
    if import_file_paths.is_empty() {
        return Err(HolonError::InvalidParameter(
            "load_holons_from_files: no import file paths provided".into(),
        ));
    }

    // Phase 1: Parse and validate all files into a single HolonLoadSet.
    //
    // For now we allow the parser to choose its own load-set key; a later
    // iteration may derive a deterministic key (e.g., from filenames or
    // a UI-provided identifier).
    let load_set_key: Option<MapString> = None;

    let load_set_reference: HolonReference =
        match parse_files_into_load_set(context.as_ref(), load_set_key, import_file_paths) {
            Ok(reference) => reference,
            Err(issues) => {
                let error = map_parsing_issues_to_holon_error(&issues);
                return Err(error);
            }
        };

    // Phase 2: Ensure we have a transient reference to the HolonLoadSet.
    //
    // The loader client constructs its graph entirely in the transient pool,
    // so we expect a `HolonReference::Transient`. If, for some reason, we
    // receive something else, treat it as an invalid parameter in this phase.
    let load_set_transient: TransientReference = match load_set_reference {
        HolonReference::Transient(tref) => tref,
        _ => {
            return Err(HolonError::InvalidParameter(
                "load_holons_from_files: expected HolonLoadSet to be a transient reference".into(),
            ));
        }
    };

    // Phase 3: Invoke the LoadHolons dance via the reference-layer helper.
    //
    // This delegates to the client holon service (`HolonServiceApi`) which
    // builds the dance request, calls into the guest (Holochain), and
    // returns a `TransientReference` to the `HolonLoadResponse` holon.
    let response_reference = load_holons(context.as_ref(), load_set_transient)?;

    Ok(response_reference)
}
