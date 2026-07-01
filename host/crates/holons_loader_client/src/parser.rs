//! Parsing entrypoints for the Holons Loader Client.
//!
//! This module is responsible for:
//! - Validating loader import JSON files against the Holon Loader JSON Schema.
//! - Deserializing into lightweight raw structures that borrow from the
//!   original buffer and expose per-holon UTF-8 byte offsets.
//! - Orchestrating per-file bundle construction via the `builder` module
//!   and wiring everything into a single `HolonLoadSet`.

use crate::builder::{
    attach_described_by_relationship, attach_relationships_for_loader_holon,
    collect_relationship_specs_for_loader_holon, create_loader_bundle_for_file,
    create_loader_holon_from_raw, normalize_ref_key, RawLoaderHolon, RawLoaderMeta,
};
use core_types::{ContentSet, FileData, ValidationError};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_prelude::prelude::*;
use json_schema_validation::json_schema_validator::validate_json_str_against_schema_str;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    context: &Arc<TransactionContext>,
    load_set_key: Option<MapString>,
    content_set: &ContentSet,
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

    for import_file in &content_set.files_to_load {
        if let Err(issue) = parse_single_import_file_into_bundle(
            context,
            &load_set_ref,
            import_file,
            &content_set.schema.raw_contents,
        ) {
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
    context: &Arc<TransactionContext>,
    load_set_key: Option<MapString>,
) -> Result<HolonReference, HolonError> {
    // Allocate a new transient holon in the current context.
    // The loader flow expects this holon to have a key; if none is provided,
    // fall back to a deterministic default.
    let key = load_set_key.unwrap_or_else(|| MapString("HolonLoadSet".to_string()));
    let transient_ref = context.mutation().new_holon(Some(key))?;

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
    context: &Arc<TransactionContext>,
    load_set_ref: &HolonReference,
    import_file: &FileData,
    schema_json: &str,
) -> Result<(), ImportFileParsingIssue> {
    let raw_json = import_file.raw_contents.as_str();
    let file_path = PathBuf::from(import_file.filename.clone());

    // 2) Validate against the loader JSON Schema and deserialize into
    //    `RawLoaderFileWithSlices<'_>`.
    let raw_file_with_slices =
        match validate_and_deserialize_loader_file(raw_json, schema_json, &import_file.filename) {
            Ok(wrapper) => wrapper,
            Err(err) => {
                return Err(ImportFileParsingIssue {
                    file_path: file_path.clone(),
                    kind: ImportFileParsingIssueKind::SchemaValidationFailure,
                    message: format!(
                    "Loader import file '{}' failed schema validation or top-level decoding: {}",
                    import_file.filename, err
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
        Path::new(&import_file.filename),
        &raw_file_with_slices.meta,
    ) {
        Ok(bundle) => bundle,
        Err(err) => {
            return Err(ImportFileParsingIssue {
                file_path: file_path.clone(),
                kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                message: format!(
                    "Failed to create HolonLoaderBundle for file '{}': {}",
                    import_file.filename, err
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
        let start_offset = compute_holon_start_offset(raw_json, holon_slice_str);

        // Decode the slice into our structured RawLoaderHolon.
        let raw_holon: RawLoaderHolon = match serde_json::from_str(holon_slice_str) {
            Ok(h) => h,
            Err(json_err) => {
                return Err(ImportFileParsingIssue {
                    file_path: file_path.clone(),
                    kind: ImportFileParsingIssueKind::JsonDecodingFailure,
                    message: format!(
                        "Failed to decode loader holon JSON in file '{}': {}",
                        import_file.filename, json_err
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
                        file_path: file_path.clone(),
                        kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                        message: format!(
                            "Failed to construct LoaderHolon '{}' from file '{}': {}",
                            raw_holon.key, import_file.filename, err
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
                file_path: file_path.clone(),
                kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                message: format!(
                    "Failed to attach relationships for LoaderHolon '{}' in file '{}': {}",
                    raw_holon.key, import_file.filename, err
                ),
                source_error: Some(err),
            });
        }

        // Attach the DescribedBy relationship if a type is present.
        if let Some(ref type_key) = raw_holon.r#type {
            let normalized_type_key = match normalize_ref_key(type_key) {
                Ok(normalized) => normalized,
                Err(err) => {
                    return Err(ImportFileParsingIssue {
                        file_path: file_path.clone(),
                        kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                        message: format!(
                            "Failed to normalize DescribedBy reference '{}' for LoaderHolon '{}' in file '{}': {}",
                            type_key,
                            raw_holon.key,
                            import_file.filename,
                            err
                        ),
                        source_error: Some(err),
                    });
                }
            };
            if let Err(err) = attach_described_by_relationship(
                context,
                &loader_holon_ref,
                &raw_holon.key,
                &normalized_type_key,
            ) {
                return Err(ImportFileParsingIssue {
                    file_path: file_path.clone(),
                    kind: ImportFileParsingIssueKind::HolonConstructionFailure,
                    message: format!(
                        "Failed to attach DescribedBy relationship for LoaderHolon '{}' in file '{}': {}",
                        raw_holon.key,
                        import_file.filename,
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
    schema_json: &str,
    filename: &str,
) -> Result<RawLoaderFileWithSlices<'a>, HolonError> {
    // 1. First run schema validation using in-memory schema + instance JSON.
    match validate_json_str_against_schema_str(schema_json, raw_json) {
        Ok(()) => { /* schema validation succeeded */ }
        Err(validation_err) => {
            // Convert ValidationError into HolonError::ValidationError
            return Err(HolonError::ValidationError(validation_err));
        }
    }

    // 2. JSON is valid; now deserialize *borrowed* RawValue slices.
    let raw_file =
        serde_json::from_str::<RawLoaderFileWithSlices<'a>>(raw_json).map_err(|err| {
            HolonError::InvalidParameter(format!(
                "Failed to decode loader import JSON after schema validation (file '{}'): {}",
                filename, err
            ))
        })?;

    validate_relationship_pair_metadata_authoring(&raw_file, filename)?;

    Ok(raw_file)
}

#[derive(Debug)]
struct RelationshipDescriptorImportMetadata {
    extends_declared_relationship_type: bool,
    extends_inverse_relationship_type: bool,
}

fn validate_relationship_pair_metadata_authoring(
    raw_file: &RawLoaderFileWithSlices<'_>,
    filename: &str,
) -> Result<(), HolonError> {
    let mut decoded_holons = Vec::with_capacity(raw_file.holons.len());
    let mut relationship_descriptors = HashMap::new();

    for raw_value in &raw_file.holons {
        let raw_holon = serde_json::from_str::<RawLoaderHolon>(raw_value.get()).map_err(|err| {
            HolonError::InvalidParameter(format!(
                "Failed to decode loader holon JSON during relationship-pair validation (file '{}'): {}",
                filename, err
            ))
        })?;

        let extends_declared_relationship_type = relationship_targets(&raw_holon, "Extends")
            .any(|target| target == "DeclaredRelationshipType");
        let extends_inverse_relationship_type = relationship_targets(&raw_holon, "Extends")
            .any(|target| target == "InverseRelationshipType");

        if extends_declared_relationship_type || extends_inverse_relationship_type {
            relationship_descriptors.insert(
                raw_holon.key.clone(),
                RelationshipDescriptorImportMetadata {
                    extends_declared_relationship_type,
                    extends_inverse_relationship_type,
                },
            );
        }

        decoded_holons.push(raw_holon);
    }

    for raw_holon in &decoded_holons {
        if let Some(inverse_of) =
            raw_holon.relationships.iter().find(|relationship| relationship.name == "InverseOf")
        {
            return Err(import_relationship_validation_error(format!(
                "Loader import file '{}' authors InverseOf on '{}'. Relationship-pair metadata must be authored from the declared relationship side with HasInverse; offending target(s): {:?}",
                filename, raw_holon.key, inverse_of.targets
            )));
        }

        let Some(metadata) = relationship_descriptors.get(&raw_holon.key) else {
            continue;
        };
        if !metadata.extends_declared_relationship_type {
            continue;
        }

        let has_inverse_targets =
            relationship_targets(raw_holon, "HasInverse").cloned().collect::<Vec<_>>();
        if has_inverse_targets.len() != 1 {
            return Err(import_relationship_validation_error(format!(
                "Declared relationship descriptor '{}' in '{}' must author exactly one HasInverse target; found {}",
                raw_holon.key,
                filename,
                has_inverse_targets.len()
            )));
        }

        let target_key = &has_inverse_targets[0];
        if let Some(target_metadata) = relationship_descriptors.get(target_key) {
            if !target_metadata.extends_inverse_relationship_type {
                return Err(import_relationship_validation_error(format!(
                    "Declared relationship descriptor '{}' in '{}' has HasInverse target '{}', but that target does not extend InverseRelationshipType",
                    raw_holon.key, filename, target_key
                )));
            }
        }
    }

    Ok(())
}

fn relationship_targets<'a>(
    raw_holon: &'a RawLoaderHolon,
    relationship_name: &'static str,
) -> impl Iterator<Item = &'a String> {
    raw_holon
        .relationships
        .iter()
        .filter(move |relationship| relationship.name == relationship_name)
        .flat_map(|relationship| relationship.targets.iter())
}

fn import_relationship_validation_error(message: String) -> HolonError {
    HolonError::ValidationError(ValidationError::RelationshipError(message))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    const BOOTSTRAP_SCHEMA: &str =
        include_str!("../../../import_files/map-schema/bootstrap-import.schema.json");

    fn relationship_pair_import(
        declared_relationships: &str,
        inverse_relationships: &str,
    ) -> String {
        format!(
            r#"{{
              "holons": [
                {{
                  "key": "(BookType)-[Authors]->(PersonType)",
                  "type": "TypeDescriptor.HolonType",
                  "properties": {{
                    "instance_type_kind": "TypeKind.Relationship"
                  }},
                  "relationships": [
                    {{ "name": "Extends", "target": {{ "$ref": "DeclaredRelationshipType" }} }}
                    {declared_relationships}
                  ]
                }},
                {{
                  "key": "(PersonType)-[AuthoredBy]->(BookType)",
                  "type": "TypeDescriptor.HolonType",
                  "properties": {{
                    "instance_type_kind": "TypeKind.Relationship"
                  }},
                  "relationships": [
                    {{ "name": "Extends", "target": {{ "$ref": "InverseRelationshipType" }} }}
                    {inverse_relationships}
                  ]
                }}
              ]
            }}"#
        )
    }

    fn assert_relationship_validation_error(
        result: Result<RawLoaderFileWithSlices<'_>, HolonError>,
    ) -> String {
        match result {
            Err(HolonError::ValidationError(ValidationError::RelationshipError(message))) => {
                message
            }
            other => panic!("expected relationship validation error, got {other:?}"),
        }
    }

    #[test]
    fn loader_import_validation_accepts_declared_has_inverse_without_inverse_of() {
        let raw_json = relationship_pair_import(
            r#",
                    {
                      "name": "HasInverse",
                      "target": {
                        "$ref": "(PersonType)-[AuthoredBy]->(BookType)"
                      }
                    }"#,
            "",
        );

        let parsed = validate_and_deserialize_loader_file(&raw_json, BOOTSTRAP_SCHEMA, "pair.json")
            .expect("declared HasInverse authoring should validate");

        assert_eq!(parsed.holons.len(), 2);
    }

    #[test]
    fn canonical_core_schema_imports_satisfy_relationship_pair_authoring_validation() {
        let core_schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("import_files")
            .join("map-schema")
            .join("core-schema");

        let mut schema_paths = fs::read_dir(&core_schema_dir)
            .unwrap_or_else(|error| {
                panic!("failed to read core schema dir {}: {error}", core_schema_dir.display())
            })
            .map(|entry| {
                entry
                    .unwrap_or_else(|error| panic!("failed to read core schema dir entry: {error}"))
                    .path()
            })
            .filter(|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some("json")
            })
            .collect::<Vec<_>>();
        schema_paths.sort();

        for schema_path in schema_paths {
            let raw_json = fs::read_to_string(&schema_path).unwrap_or_else(|error| {
                panic!("failed to read core schema export {}: {error}", schema_path.display())
            });
            validate_and_deserialize_loader_file(
                &raw_json,
                BOOTSTRAP_SCHEMA,
                &schema_path.display().to_string(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "core schema export {} failed relationship-pair validation: {error}",
                    schema_path.display()
                )
            });
        }
    }

    #[test]
    fn loader_import_validation_rejects_declared_relationship_without_has_inverse() {
        let raw_json = relationship_pair_import("", "");

        let message = assert_relationship_validation_error(validate_and_deserialize_loader_file(
            &raw_json,
            BOOTSTRAP_SCHEMA,
            "missing-has-inverse.json",
        ));

        assert!(message.contains("must author exactly one HasInverse target"));
        assert!(message.contains("found 0"));
    }

    #[test]
    fn loader_import_validation_rejects_has_inverse_target_with_wrong_kind() {
        let raw_json = relationship_pair_import(
            r#",
                    {
                      "name": "HasInverse",
                      "target": {
                        "$ref": "(PersonType)-[AuthoredBy]->(BookType)"
                      }
                    }"#,
            "",
        )
        .replace("InverseRelationshipType", "DeclaredRelationshipType");

        let message = assert_relationship_validation_error(validate_and_deserialize_loader_file(
            &raw_json,
            BOOTSTRAP_SCHEMA,
            "wrong-kind-has-inverse.json",
        ));

        assert!(message.contains("does not extend InverseRelationshipType"));
    }

    #[test]
    fn loader_import_validation_rejects_authored_inverse_of_relationship() {
        let raw_json = relationship_pair_import(
            r#",
                    {
                      "name": "HasInverse",
                      "target": {
                        "$ref": "(PersonType)-[AuthoredBy]->(BookType)"
                      }
                    }"#,
            r#",
                    {
                      "name": "InverseOf",
                      "target": {
                        "$ref": "(BookType)-[Authors]->(PersonType)"
                      }
                    }"#,
        );

        let message = assert_relationship_validation_error(validate_and_deserialize_loader_file(
            &raw_json,
            BOOTSTRAP_SCHEMA,
            "authored-inverse-of.json",
        ));

        assert!(message.contains("authors InverseOf"));
        assert!(message.contains("HasInverse"));
    }
}
