use crate::types::ImportFileParsingIssue;

use base_types::MapString;
use core_types::HolonError;
use holons_core::{HolonReference, HolonsContextBehavior};

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Raw JSON representation of a loader import file as defined by the loader schema.
#[derive(Debug, Deserialize)]
pub struct RawLoaderFile {
    pub meta: Option<RawLoaderMeta>,
    pub holons: Vec<RawLoaderHolon>,
}

#[derive(Debug, Deserialize)]
pub struct RawLoaderMeta {
    /// Optional explicit bundle key override.
    pub bundle_key: Option<String>,

    /// Additional metadata fields (e.g., filename, notes, etc.).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Raw JSON representation of a single loader holon record.
#[derive(Debug, Deserialize)]
pub struct RawLoaderHolon {
    /// Loader holon key used as `LoaderHolon.key`.
    pub key: String,

    /// Domain properties for this holon instance.
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,

    /// Optional type descriptor key, e.g., "Book.HolonType".
    #[serde(default)]
    pub r#type: Option<String>,

    /// Declared relationships (forward side in JSON).
    #[serde(default)]
    pub relationships: Vec<RawRelationshipEndpoints>,

    /// Inverse (“embedded”) relationships.
    #[serde(default)]
    pub embedded_inverse_relationships: Vec<RawRelationshipEndpoints>,
}

/// Shared endpoints shape for declared and inverse relationships in the JSON.
#[derive(Debug, Deserialize)]
pub struct RawRelationshipEndpoints {
    pub name: String,
    pub targets: Vec<String>,
}

/// Internal unified representation of a loader relationship, with an explicit
/// `is_declared` flag used to set `LoaderRelationshipReference.is_declared`.
pub struct RawRelationshipSpec {
    pub name: String,
    pub targets: Vec<String>,
    /// `true` for declared relationships, `false` for inverse ones.
    pub is_declared: bool,
}

/// High-level entrypoint for parsing all import files into a single `HolonLoadSet`.
///
/// - Creates a `HolonLoadSet` holon.
/// - For each file:
///   - Creates a `HolonLoaderBundle`.
///   - Attaches the bundle to the load set via `Contains`.
///   - Parses `LoaderHolon`s and their relationships.
/// - Returns a `HolonReference` pointing to the `HolonLoadSet` on success.
/// - Returns `Err(Vec<ImportFileParsingIssue>)` if any parsing error occurs.
pub fn parse_files_into_load_set(
    context: &dyn HolonsContextBehavior,
    load_set_key: Option<MapString>,
    import_file_paths: &[PathBuf],
) -> Result<HolonReference, Vec<ImportFileParsingIssue>>;

/// Create an empty `HolonLoadSet` holon and return its reference.
pub fn create_holon_load_set(
    context: &dyn HolonsContextBehavior,
    load_set_key: Option<MapString>,
) -> Result<HolonReference, HolonError>;

/// Parse a single import file and attach its bundle + loader holons
/// to the existing `HolonLoadSet`.
///
/// Returns `Ok(())` on success; `Err(ImportFileParsingIssue)` for a per-file failure.
pub fn parse_single_import_file_into_bundle(
    context: &dyn HolonsContextBehavior,
    load_set_ref: &HolonReference,
    import_file_path: &PathBuf,
) -> Result<(), ImportFileParsingIssue>;

/// Create a `HolonLoaderBundle` holon for a single import file and
/// attach it to the `HolonLoadSet` via `Contains`.
pub fn create_loader_bundle_for_file(
    context: &dyn HolonsContextBehavior,
    load_set_ref: &HolonReference,
    import_file_path: &Path,
    raw_meta: &Option<RawLoaderMeta>,
) -> Result<HolonReference, HolonError>;

/// Create a `LoaderHolon` for a single `RawLoaderHolon`, set its properties
/// (including `start_utf8_byte_offset`), and attach it to the bundle via `BundleMembers`.
pub fn create_loader_holon_from_raw(
    context: &dyn HolonsContextBehavior,
    bundle_ref: &HolonReference,
    raw_holon: &RawLoaderHolon,
    start_utf8_byte_offset: i64,
) -> Result<HolonReference, HolonError>;

/// Convert the declared and embedded inverse relationship arrays from a `RawLoaderHolon`
/// into a unified list of `RawRelationshipSpec` values annotated with `is_declared`.
pub fn collect_relationship_specs_for_loader_holon(
    raw_holon: &RawLoaderHolon,
) -> Vec<RawRelationshipSpec>;

/// Create `LoaderRelationshipReference` holons and their endpoint `LoaderHolonReference`
/// holons for all specified relationships (both declared and inverse).
///
/// Each `RawRelationshipSpec` determines the value of `is_declared` on the
/// resulting `LoaderRelationshipReference`.
pub fn attach_relationships_for_loader_holon(
    context: &dyn HolonsContextBehavior,
    loader_holon_ref: &HolonReference,
    source_holon_key: &str,
    relationship_specs: &[RawRelationshipSpec],
) -> Result<(), HolonError>;

/// Create a `LoaderRelationshipReference` of type `DescribedBy` for the
/// given loader holon and attach it correctly if a `type` is specified.
pub fn attach_described_by_relationship(
    context: &dyn HolonsContextBehavior,
    loader_holon_ref: &HolonReference,
    holon_key: &str,
    type_descriptor_key: &str,
) -> Result<(), HolonError>;
