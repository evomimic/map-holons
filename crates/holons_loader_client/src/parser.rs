//! Parsing entrypoints for the Holons Loader Client.
//!
//! This module is responsible for:
//! - Reading loader import JSON files from disk.
//! - Validating them against the Holon Loader JSON Schema.
//! - Deserializing into lightweight raw structures that borrow from the
//!   original buffer and expose per-holon UTF-8 byte offsets.
//! - Orchestrating per-file bundle construction via the `builder` module
//!   and wiring everything into a single `HolonLoadSet`.

use std::fs;
use std::path::{Path, PathBuf};

use holons_prelude::prelude::*;
use json_schema_validation::json_schema_validator::validate_json_against_schema;
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::builder::{
    attach_described_by_relationship, attach_relationships_for_loader_holon,
    collect_relationship_specs_for_loader_holon, create_loader_bundle_for_file,
    create_loader_holon_from_raw, normalize_ref_key, RawLoaderHolon, RawLoaderMeta,
};

/// Canonical on-disk path to the Holon Loader JSON Schema.
///
/// In the initial implementation this is a constant; a later iteration may
/// load this from configuration or a per-space registry.
pub const BOOTSTRAP_IMPORT_SCHEMA_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../import_files/map-schema/bootstrap-import.schema.json"
);

/// High-level classification of a per-file parsing issue.
///
/// This allows the caller (and UI) to distinguish between simple I/O failures,
/// schema violations, JSON decoding problems, and internal holon construction
/// failures.
#[derive(Debug)]
pub enum ImportFileParsingIssueKind {
    /// Any failure to read or open the import file.
    IoFailure,

    /// JSON Schema validation errors (structure or type mismatches).
    SchemaValidationFailure,

    /// Raw JSON decoding errors (malformed JSON or unexpected shape).
    JsonDecodingFailure,

    /// Errors that occur while constructing transient holons and their
    /// relationships inside the loader graph.
    HolonConstructionFailure,
}

/// Represents a single problem encountered while processing one import file.
///
/// The parser never panics on a single bad file; instead it aggregates these
/// issues so the UI can report them alongside any successful bundles.
#[derive(Debug)]
pub struct ImportFileParsingIssue {
    /// The path of the import file that failed.
    pub file_path: PathBuf,

    /// High-level classification of the failure.
    pub kind: ImportFileParsingIssueKind,

    /// Human-readable explanation suitable for logs or UI.
    pub message: String,

    /// Optional underlying HolonError when the failure originates from
    /// the loader / holon layer rather than raw I/O / JSON.
    pub source_error: Option<HolonError>,
}

/// Raw JSON representation of a loader import file as defined by the loader
/// schema, using borrowed `RawValue` slices for holons.
///
/// This first-pass wrapper allows us to compute per-holon byte offsets
/// relative to the original JSON buffer, and then re-parse each holon slice
/// into `RawLoaderHolon` for graph construction.
#[derive(Debug, Deserialize)]
pub struct RawLoaderFileWithSlices<'a> {
    /// Optional metadata block attached to this bundle.
    pub meta: Option<RawLoaderMeta>,

    /// Borrowed raw JSON fragments for each loader holon record.
    ///
    /// Each `&RawValue` is a view into the original file buffer; we use this
    /// to compute `start_utf8_byte_offset` via pointer arithmetic before
    /// deserializing into `RawLoaderHolon`.
    #[serde(borrow)]
    pub holons: Vec<&'a RawValue>,
}

/// High-level entrypoint for parsing all import files into a single
/// `HolonLoadSet` holon graph.
///
/// Behavior:
/// - Creates (or reuses) a `HolonLoadSet` holon.
/// - For each import file:
///   - Validates the JSON against the loader schema.
///   - Deserializes into `RawLoaderFileWithSlices<'_>`.
///   - Computes UTF-8 byte offsets per holon.
///   - Delegates to the builder module to construct:
///       - a `HolonLoaderBundle` per file, and
///       - `LoaderHolon` + relationship references per record.
/// - Returns a `HolonReference` pointing to the `HolonLoadSet` on success.
/// - Returns `Err(Vec<ImportFileParsingIssue>)` if any file fails.
///
/// The caller decides whether a single-file failure should abort the entire
/// load or allow partial success; this function simply reports all issues.
pub fn parse_files_into_load_set(
    context: &dyn HolonsContextBehavior,
    load_set_key: Option<MapString>,
    import_file_paths: &[PathBuf],
) -> Result<HolonReference, Vec<ImportFileParsingIssue>> {
    // 1) Create the HolonLoadSet container holon.
    //
    // If we cannot even create the load set, there is no meaningful way to
    // proceed with per-file parsing. We return a single HolonConstructionFailure
    // issue that describes the problem.
    let load_set_ref = match create_holon_load_set(context, load_set_key) {
        Ok(reference) => reference,
        Err(err) => {
            let issue = ImportFileParsingIssue {
                file_path: PathBuf::from("<HolonLoadSet>"),
                kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                message: "Failed to create HolonLoadSet holon".to_string(),
                source_error: Some(err),
            };
            return Err(vec![issue]);
        }
    };

    // 2) Attempt to parse each import file into its own bundle.
    //
    // We collect all per-file issues rather than failing fast so the caller
    // can report a comprehensive summary to the user.
    let mut issues: Vec<ImportFileParsingIssue> = Vec::new();

    for import_path in import_file_paths {
        if let Err(issue) =
            parse_single_import_file_into_bundle(context, &load_set_ref, import_path)
        {
            issues.push(issue);
        }
    }

    // 3) If any file failed, return the full set of issues. Otherwise,
    //    return the HolonLoadSet reference.
    if issues.is_empty() {
        Ok(load_set_ref)
    } else {
        Err(issues)
    }
}

/// Create an empty `HolonLoadSet` holon and return its reference.
///
/// The returned holon acts as the container for all `HolonLoaderBundle`
/// instances created during a single loader invocation.
///
/// This uses the generic `new_holon` helper from the MAP prelude, which
/// creates a transient holon with an optional explicit key.
pub fn create_holon_load_set(
    context: &dyn HolonsContextBehavior,
    load_set_key: Option<MapString>,
) -> Result<HolonReference, HolonError> {
    // Allocate a new transient holon in the current context.
    // The loader flow expects this holon to have a key; if none is provided,
    // fall back to a deterministic default.
    let key = load_set_key.unwrap_or_else(|| MapString("HolonLoadSet".to_string()));
    let transient_ref = new_holon(context, Some(key))?;

    Ok(HolonReference::Transient(transient_ref))
}

/// Parse a single import file and attach its `HolonLoaderBundle` and
/// member `LoaderHolon`s to the existing `HolonLoadSet`.
///
/// This function is responsible for:
/// - Reading the file into memory.
/// - Validating the JSON instance against the loader schema.
/// - Deserializing into `RawLoaderFileWithSlices<'_>`.
/// - Computing `start_utf8_byte_offset` for each holon record.
/// - Delegating bundle + holon construction to the builder module.
///
/// On success, the `HolonLoadSet` now contains a new bundle for this file.
/// On failure, a single `ImportFileParsingIssue` is returned.
pub fn parse_single_import_file_into_bundle(
    context: &dyn HolonsContextBehavior,
    load_set_ref: &HolonReference,
    import_file_path: &PathBuf,
) -> Result<(), ImportFileParsingIssue> {
    // 1) Read the entire file into memory as a UTF-8 string.
    let raw_json = match fs::read_to_string(import_file_path) {
        Ok(text) => text,
        Err(io_err) => {
            return Err(ImportFileParsingIssue {
                file_path: import_file_path.clone(),
                kind: ImportFileParsingIssueKind::IoFailure,
                message: format!(
                    "Failed to read import file '{}': {}",
                    import_file_path.display(),
                    io_err
                ),
                source_error: None,
            });
        }
    };

    // 2) Validate against the loader JSON Schema and deserialize into
    //    `RawLoaderFileWithSlices<'_>`.
    let raw_file_with_slices =
        match validate_and_deserialize_loader_file(&raw_json, import_file_path.as_path()) {
            Ok(wrapper) => wrapper,
            Err(err) => {
                // Treat *any* HolonError from validation/deserialization as
                // a schema validation failure at this layer. The underlying
                // cause is preserved on the issue.
                return Err(ImportFileParsingIssue {
                    file_path: import_file_path.clone(),
                    kind: ImportFileParsingIssueKind::SchemaValidationFailure,
                    message: format!(
                    "Loader import file '{}' failed schema validation or top-level decoding: {}",
                    import_file_path.display(),
                    err
                ),
                    source_error: Some(err),
                });
            }
        };

    // 3) Create the HolonLoaderBundle for this file and attach it to
    //    the HolonLoadSet.
    let bundle_ref = match create_loader_bundle_for_file(
        context,
        load_set_ref,
        import_file_path.as_path(),
        &raw_file_with_slices.meta,
    ) {
        Ok(bundle) => bundle,
        Err(err) => {
            return Err(ImportFileParsingIssue {
                file_path: import_file_path.clone(),
                kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                message: format!(
                    "Failed to create HolonLoaderBundle for file '{}': {}",
                    import_file_path.display(),
                    err
                ),
                source_error: Some(err),
            });
        }
    };

    // 4) For each holon slice:
    //    - compute byte offset
    //    - parse into RawLoaderHolon
    //    - create LoaderHolon + relationships via builder.
    for raw_value in raw_file_with_slices.holons {
        // Compute the 0-based UTF-8 byte offset for this holon slice.
        let holon_slice_str = raw_value.get();
        let start_offset = compute_holon_start_offset(&raw_json, holon_slice_str);

        // Decode the slice into our structured RawLoaderHolon.
        let raw_holon: RawLoaderHolon = match serde_json::from_str(holon_slice_str) {
            Ok(h) => h,
            Err(json_err) => {
                return Err(ImportFileParsingIssue {
                    file_path: import_file_path.clone(),
                    kind: ImportFileParsingIssueKind::JsonDecodingFailure,
                    message: format!(
                        "Failed to decode loader holon JSON in file '{}': {}",
                        import_file_path.display(),
                        json_err
                    ),
                    source_error: None,
                });
            }
        };

        // Create the LoaderHolon and attach it to the bundle.
        let loader_holon_ref =
            match create_loader_holon_from_raw(context, &bundle_ref, &raw_holon, start_offset) {
                Ok(r) => r,
                Err(err) => {
                    return Err(ImportFileParsingIssue {
                        file_path: import_file_path.clone(),
                        kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                        message: format!(
                            "Failed to construct LoaderHolon '{}' from file '{}': {}",
                            raw_holon.key,
                            import_file_path.display(),
                            err
                        ),
                        source_error: Some(err),
                    });
                }
            };

        // Collect relationship specs from the unified relationships array.
        let relationship_specs = collect_relationship_specs_for_loader_holon(&raw_holon);

        // Attach relationship references for all relationships.
        if let Err(err) = attach_relationships_for_loader_holon(
            context,
            &loader_holon_ref,
            &raw_holon.key,
            &relationship_specs,
        ) {
            return Err(ImportFileParsingIssue {
                file_path: import_file_path.clone(),
                kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                message: format!(
                    "Failed to attach relationships for LoaderHolon '{}' in file '{}': {}",
                    raw_holon.key,
                    import_file_path.display(),
                    err
                ),
                source_error: Some(err),
            });
        }

        // Attach the DescribedBy relationship if a type is present.
        if let Some(ref type_key) = raw_holon.r#type {
            let normalized_type_key = normalize_ref_key(type_key);
            if let Err(err) = attach_described_by_relationship(
                context,
                &loader_holon_ref,
                &raw_holon.key,
                &normalized_type_key,
            ) {
                return Err(ImportFileParsingIssue {
                    file_path: import_file_path.clone(),
                    kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                    message: format!(
                        "Failed to attach DescribedBy relationship for LoaderHolon '{}' in file '{}': {}",
                        raw_holon.key,
                        import_file_path.display(),
                        err
                    ),
                    source_error: Some(err),
                });
            }
        }
    }

    Ok(())
}

/// Validate a raw JSON buffer against the Holon Loader JSON Schema and
/// deserialize it into a `RawLoaderFileWithSlices<'a>` wrapper.
///
/// Phase 1 behavior:
/// - Run schema validation using the `json_schema_validation` crate.
/// - On success, deserialize into `RawLoaderFileWithSlices`.
/// - On failure, return `HolonError::ValidationError`.
pub fn validate_and_deserialize_loader_file<'a>(
    raw_json: &'a str,
    import_file_path: &Path,
) -> Result<RawLoaderFileWithSlices<'a>, HolonError> {
    // 1. First run schema validation using the on-disk schema file.
    // NOTE: If BOOTSTRAP_IMPORT_SCHEMA_PATH is relative, it must be relative
    //       to the working directory where the host is run, usually the
    //       Conductora root.
    let schema_path = Path::new(BOOTSTRAP_IMPORT_SCHEMA_PATH);

    match validate_json_against_schema(schema_path, import_file_path) {
        Ok(()) => { /* schema validation succeeded */ }
        Err(validation_err) => {
            // Convert ValidationError into HolonError::ValidationError
            return Err(HolonError::ValidationError(validation_err));
        }
    }

    // 2. JSON is valid; now deserialize *borrowed* RawValue slices.
    serde_json::from_str::<RawLoaderFileWithSlices<'a>>(raw_json).map_err(|err| {
        HolonError::InvalidParameter(format!(
            "Failed to decode loader import JSON after schema validation: {}",
            err
        ))
    })
}

/// Convenience helper for computing a 0-based UTF-8 byte offset into a file
/// for a given holon slice.
///
/// The caller is expected to pass:
/// - `file_buffer`: the complete JSON file contents, and
/// - `holon_slice`: the `&str` backing a `RawValue` that is guaranteed to be
///   a slice into `file_buffer`.
///
/// This function encapsulates the pointer arithmetic so that any future
/// change to offset computation (e.g., using `serde_spanned`) is localized.
pub fn compute_holon_start_offset(file_buffer: &str, holon_slice: &str) -> i64 {
    let base_ptr = file_buffer.as_ptr() as usize;
    let slice_ptr = holon_slice.as_ptr() as usize;

    let offset =
        slice_ptr.checked_sub(base_ptr).expect("holon_slice must be a slice into file_buffer");

    offset as i64
}
