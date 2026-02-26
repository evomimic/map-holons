//! Graph construction helpers for the Holons Loader Client.
//!
//! This module is responsible for transforming parsed JSON loader records
//! into in-memory transient holons that match the loader schema graph:
//!
//! - `HolonLoadSet`
//! - `HolonLoaderBundle`
//! - `LoaderHolon`
//! - `LoaderRelationshipReference`
//! - `LoaderHolonReference`
//!
//! It exposes small, focused functions that the parsing layer can call after
//! validating and decoding the raw JSON into `RawLoaderHolon` structures.

use holons_prelude::prelude::*;
use serde::{de, Deserialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use holons_core::core_shared_objects::transactions::TransactionContext;

/// Raw JSON representation of the `"meta"` block from a loader import file.
///
/// This struct captures any explicit bundle key override as well as additional
/// metadata that should be attached as properties on the `HolonLoaderBundle`
/// (e.g., filename, notes, generator info).
#[derive(Debug, Deserialize)]
pub struct RawLoaderMeta {
    /// Optional explicit bundle key override supplied by the JSON.
    pub bundle_key: Option<String>,

    /// Additional metadata fields (e.g., filename, notes, generator details).
    ///
    /// These are flattened onto the top-level `"meta"` object and later
    /// projected into properties on the bundle holon.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Raw JSON representation of a single loader holon record.
///
/// This struct matches the shape of entries in the `"holons"` array of a
/// loader import file, as defined by the Holon Loader JSON Schema.
#[derive(Debug, Deserialize)]
pub struct RawLoaderHolon {
    /// Loader holon key used as `LoaderHolon.key`.
    pub key: String,

    /// Domain properties for this holon instance.
    ///
    /// These are later projected onto real holons by the loader mapper.
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,

    /// Optional type descriptor key, e.g., `"Book.HolonType"`.
    ///
    /// When present, the builder will create a `LoaderRelationshipReference`
    /// of type `DescribedBy` that targets this type descriptor.
    #[serde(default)]
    pub r#type: Option<String>,

    /// Relationships for this holon instance.
    ///
    /// The `name` is the relationship type name (`type_name`), and
    /// `targets` are the endpoint holon keys. Directionality (declared vs
    /// inverse) will ultimately be derived from the relationship type
    /// definition (Extends → DeclaredRelationshipType / InverseRelationshipType)
    /// when constructing `LoaderRelationshipReference` holons. In the initial
    /// implementation we treat all relationships as declared (`is_declared = true`).
    #[serde(default)]
    pub relationships: Vec<RawRelationshipEndpoints>,
}

/// Shared endpoints shape for relationships in the JSON.
///
/// This mirrors the structure used in the loader schema:
/// ```json
/// { "name": "AuthoredBy", "target": ["Person:Alice", "Person:Bob"] }
/// ```
#[derive(Debug, Deserialize)]
pub struct RawRelationshipEndpoints {
    /// The loader relationship name (declared or inverse).
    pub name: String,

    /// Ordered list of holon keys that participate as targets.
    #[serde(rename = "target", deserialize_with = "deserialize_targets")]
    pub targets: Vec<String>,
}

/// Internal unified representation of a loader relationship.
///
/// The parser converts `RawLoaderHolon.relationships` into this neutral shape
/// so that the builder logic can operate over a single collection. Polarity
/// (`is_declared`) is *not* stored here; it will be derived from the
/// relationship type definitions in a later phase.
pub struct RawRelationshipSpec {
    /// Relationship name, as declared in the JSON.
    pub name: String,

    /// Ordered list of holon keys that should be used as targets.
    pub targets: Vec<String>,
}

/// Create a `HolonLoaderBundle` holon for a single import file and attach it
/// to the provided `HolonLoadSet` via the `Contains` relationship.
///
/// Responsibilities:
/// - Determine the bundle key (from `RawLoaderMeta.bundle_key` or filename).
/// - Create a new transient `HolonLoaderBundle`.
/// - Populate bundle metadata properties (including `Filename`).
/// - Attach the bundle to the `HolonLoadSet` with `Contains`.
pub fn create_loader_bundle_for_file(
    context: &Arc<TransactionContext>,
    load_set_ref: &HolonReference,
    import_file_path: &Path,
    raw_meta: &Option<RawLoaderMeta>,
) -> Result<HolonReference, HolonError> {
    // Determine the bundle key (explicit override takes precedence).
    let bundle_key = derive_bundle_key(import_file_path, raw_meta);
    let mut bundle = context.mutation().new_holon(Some(MapString(bundle_key)))?;

    // Apply any metadata properties from the JSON meta block.
    if let Some(meta) = raw_meta {
        apply_json_properties(&mut bundle, &meta.extra)?;
    }

    // Always stamp the canonical Filename property with the provided file path.
    let filename_value = path_to_string(import_file_path);
    bundle.with_property_value(
        CorePropertyTypeName::Filename,
        BaseValue::StringValue(MapString(filename_value)),
    )?;

    // Attach bundle to the HolonLoadSet via Contains.
    let bundle_reference = HolonReference::Transient(bundle);
    let mut load_set = load_set_ref.clone();
    load_set
        .add_related_holons(CoreRelationshipTypeName::Contains, vec![bundle_reference.clone()])?;

    Ok(bundle_reference)
}

/// Create a `LoaderHolon` for a single `RawLoaderHolon`, set its properties
/// (including `start_utf8_byte_offset`), and attach it to the bundle via
/// the `BundleMembers` relationship.
///
/// Responsibilities:
/// - Create a new transient `LoaderHolon` with the given key.
/// - Copy all domain `properties` from the raw record.
/// - Set the `StartUtf8ByteOffset` property from the provided offset.
/// - Attach the loader holon to its `HolonLoaderBundle`.
pub fn create_loader_holon_from_raw(
    context: &Arc<TransactionContext>,
    bundle_ref: &HolonReference,
    raw_holon: &RawLoaderHolon,
    start_utf8_byte_offset: i64,
) -> Result<HolonReference, HolonError> {
    // Ensure we have a key to attach relationships/properties to.
    if raw_holon.key.trim().is_empty() {
        return Err(HolonError::InvalidParameter("LoaderHolon key cannot be empty".into()));
    }

    let mut loader_holon = context.mutation().new_holon(Some(MapString(raw_holon.key.clone())))?;

    // Copy user-defined properties.
    apply_json_properties(&mut loader_holon, &raw_holon.properties)?;

    // Stamp the loader-only byte offset provenance property.
    loader_holon.with_property_value(
        CorePropertyTypeName::StartUtf8ByteOffset,
        start_utf8_byte_offset.to_base_value(),
    )?;

    let loader_ref = HolonReference::Transient(loader_holon.clone());

    // Attach the loader holon to its bundle via BundleMembers.
    let mut bundle = bundle_ref.clone();
    bundle.add_related_holons(CoreRelationshipTypeName::BundleMembers, vec![loader_ref.clone()])?;

    Ok(loader_ref)
}

/// Convert the relationships array from a `RawLoaderHolon` into a unified
/// list of `RawRelationshipSpec` values.
///
/// This allows the rest of the builder logic to operate over a single
/// collection regardless of how the JSON was produced. Directionality
/// (`is_declared`) is computed later from relationship type definitions.
pub fn collect_relationship_specs_for_loader_holon(
    raw_holon: &RawLoaderHolon,
) -> Vec<RawRelationshipSpec> {
    raw_holon
        .relationships
        .iter()
        .map(|rel| RawRelationshipSpec { name: rel.name.clone(), targets: rel.targets.clone() })
        .collect()
}

/// Create `LoaderRelationshipReference` holons and their endpoint
/// `LoaderHolonReference` holons for all specified relationships.
///
/// For each `RawRelationshipSpec`:
/// - Create a `LoaderRelationshipReference` and set:
///   - `relationship_name`
///   - `is_declared` (currently always `true`; a later phase will derive this
///     from relationship type definitions).
/// - Create a `LoaderHolonReference` for the source holon key.
/// - Create one `LoaderHolonReference` per target holon key (in order).
/// - Wire relationships:
///   - `HasRelationshipReference` (LoaderHolon → LRR)
///   - `ReferenceSource` (LRR → source endpoint)
///   - `ReferenceTarget` (LRR → target endpoints*)
pub fn attach_relationships_for_loader_holon(
    context: &Arc<TransactionContext>,
    loader_holon_ref: &HolonReference,
    source_holon_key: &str,
    relationship_specs: &[RawRelationshipSpec],
) -> Result<(), HolonError> {
    if relationship_specs.is_empty() {
        return Ok(());
    }

    let mut loader_holon = loader_holon_ref.clone();
    for spec in relationship_specs {
        if spec.targets.is_empty() {
            return Err(HolonError::InvalidParameter(format!(
                "Relationship '{}' for loader holon '{}' must specify at least one target",
                spec.name, source_holon_key
            )));
        }

        create_relationship_reference(
            context,
            &mut loader_holon,
            source_holon_key,
            &spec.name,
            &spec.targets,
        )?;
    }

    Ok(())
}

/// Create a `LoaderRelationshipReference` of type `DescribedBy` for the
/// given loader holon and attach it correctly if a `type` is specified.
///
/// Behavior:
/// - If `type_descriptor_key` is non-empty:
///   - Create `LoaderRelationshipReference` with `relationship_name = "DescribedBy"`.
///   - Mark `is_declared = true`.
///   - Create source and target `LoaderHolonReference`s with the appropriate
///     `holon_key` values.
///   - Wire them using:
///       - `HasRelationshipReference`
///       - `ReferenceSource`
///       - `ReferenceTarget`
/// - If `type_descriptor_key` is empty, this function is a no-op.
pub fn attach_described_by_relationship(
    context: &Arc<TransactionContext>,
    loader_holon_ref: &HolonReference,
    holon_key: &str,
    type_descriptor_key: &str,
) -> Result<(), HolonError> {
    if type_descriptor_key.trim().is_empty() {
        return Ok(());
    }

    let mut loader_holon = loader_holon_ref.clone();
    let targets = vec![type_descriptor_key.to_string()];
    let described_by_name = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
    let described_by_string = described_by_name.to_string();
    create_relationship_reference(
        context,
        &mut loader_holon,
        holon_key,
        described_by_string.as_str(),
        &targets,
    )
}

/// Derive the bundle key for an import file, honoring explicit overrides when present.
fn derive_bundle_key(import_file_path: &Path, raw_meta: &Option<RawLoaderMeta>) -> String {
    if let Some(meta) = raw_meta {
        if let Some(bundle_key) = meta.bundle_key.as_ref() {
            let trimmed = bundle_key.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    let filename =
        import_file_path.file_name().and_then(|name| name.to_str()).unwrap_or("ImportFile");

    format!("Bundle.{filename}")
}

/// Convert a `Path` to an owned UTF-8 string suitable for holon properties.
fn path_to_string(import_file_path: &Path) -> String {
    import_file_path
        .to_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| import_file_path.display().to_string())
}

/// Apply a JSON property map onto a transient holon by converting each value into a `BaseValue`.
fn apply_json_properties(
    target: &mut TransientReference,
    properties: &HashMap<String, Value>,
) -> Result<(), HolonError> {
    if properties.is_empty() {
        return Ok(());
    }

    for (name, value) in properties {
        let base_value = json_value_to_base_value(name, value)?;
        target.with_property_value(name.as_str(), base_value)?;
    }

    Ok(())
}

// We could add impl ToBaseValue for serde_json::Value, but that seems too broad for now.
/// Convert a `serde_json::Value` into a `BaseValue` understood by the holon layer.
fn json_value_to_base_value(property_name: &str, value: &Value) -> Result<BaseValue, HolonError> {
    match value {
        Value::String(s) => Ok(BaseValue::StringValue(MapString(s.clone()))),
        Value::Bool(b) => Ok(BaseValue::BooleanValue(MapBoolean(*b))),
        Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                Ok(BaseValue::IntegerValue(MapInteger(i)))
            } else if let Some(u) = num.as_u64() {
                if u <= i64::MAX as u64 {
                    Ok(BaseValue::IntegerValue(MapInteger(u as i64)))
                } else {
                    Err(HolonError::InvalidParameter(format!(
                        "Property '{}' value '{}' exceeds supported integer range",
                        property_name, value
                    )))
                }
            } else {
                Err(HolonError::InvalidParameter(format!(
                    "Property '{}' numeric value '{}' must be an integer",
                    property_name, value
                )))
            }
        }
        // For loader meta fields we occasionally receive arrays/objects; serialize them
        // to a JSON string to preserve the content without rejecting the file.
        Value::Array(_) | Value::Object(_) | Value::Null => {
            Ok(BaseValue::StringValue(MapString(value.to_string())))
        }
    }
}

/// Normalize a relationship target reference string into a canonical holon key.
/// Currently strips local-ref style prefixes like `#` or `id:`; extend as needed.
pub fn normalize_ref_key(raw: &str) -> String {
    if let Some(stripped) = raw.strip_prefix('#') {
        stripped.to_string()
    } else if let Some(stripped) = raw.strip_prefix("id:") {
        stripped.to_string()
    } else {
        raw.to_string()
    }
}

/// Accepts relationship targets in several forms:
/// - single string
/// - array of strings
/// - single object with `$ref`
/// - array of `$ref` objects
fn deserialize_targets<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;

    fn convert_target(val: Value) -> Result<String, String> {
        match val {
            Value::String(s) => Ok(normalize_ref_key(&s)),
            Value::Object(map) => map
                .get("$ref")
                .and_then(|v| v.as_str())
                .map(|s| normalize_ref_key(s))
                .ok_or_else(|| format!("target object missing '$ref': {map:?}")),
            other => Err(format!("unsupported target value: {other}")),
        }
    }

    match value {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(convert_target(item).map_err(de::Error::custom)?);
            }
            Ok(out)
        }
        other => Ok(vec![convert_target(other).map_err(de::Error::custom)?]),
    }
}

/// Create the loader relationship + endpoint holons and wire them into the graph.
fn create_relationship_reference(
    context: &Arc<TransactionContext>,
    loader_holon_ref: &mut HolonReference,
    source_holon_key: &str,
    relationship_name: &str,
    target_keys: &[String],
) -> Result<(), HolonError> {
    if source_holon_key.trim().is_empty() {
        return Err(HolonError::InvalidParameter(
            "Relationship source holon key cannot be empty".into(),
        ));
    }

    if target_keys.is_empty() {
        return Err(HolonError::InvalidParameter(format!(
            "Relationship '{}' for loader holon '{}' requires at least one target key",
            relationship_name, source_holon_key
        )));
    }

    // Relationship container holon.
    let relationship_key =
        format!("LoaderRelationshipReference.{}.{}", source_holon_key, relationship_name);
    let mut relationship_reference =
        context.mutation().new_holon(Some(MapString(relationship_key)))?;

    relationship_reference.with_property_value(
        CorePropertyTypeName::RelationshipName,
        BaseValue::StringValue(MapString(relationship_name.to_string())),
    )?;
    relationship_reference.with_property_value(
        CorePropertyTypeName::IsDeclared,
        BaseValue::BooleanValue(MapBoolean(true)),
    )?;

    // Source endpoint reference.
    let source_ref_key = format!("LoaderHolonReference.Source.{}", source_holon_key);
    let mut source_reference = context.mutation().new_holon(Some(MapString(source_ref_key)))?;
    source_reference.with_property_value(
        CorePropertyTypeName::HolonKey,
        BaseValue::StringValue(MapString(source_holon_key.to_string())),
    )?;

    // Target endpoint references.
    let mut target_references: Vec<HolonReference> = Vec::with_capacity(target_keys.len());
    for (index, target_key) in target_keys.iter().enumerate() {
        if target_key.trim().is_empty() {
            return Err(HolonError::InvalidParameter(format!(
                "Relationship '{}' for loader holon '{}' contains an empty target key",
                relationship_name, source_holon_key
            )));
        }

        let target_ref_key = format!("LoaderHolonReference.Target{}.{}", index + 1, target_key);
        let mut target_reference = context.mutation().new_holon(Some(MapString(target_ref_key)))?;
        target_reference.with_property_value(
            CorePropertyTypeName::HolonKey,
            BaseValue::StringValue(MapString(target_key.clone())),
        )?;
        target_references.push(HolonReference::Transient(target_reference));
    }

    // Wire endpoints to the relationship reference.
    relationship_reference.add_related_holons(
        CoreRelationshipTypeName::ReferenceSource,
        vec![HolonReference::Transient(source_reference)],
    )?;
    relationship_reference
        .add_related_holons(CoreRelationshipTypeName::ReferenceTarget, target_references)?;

    // Attach the relationship reference to the loader holon.
    loader_holon_ref.add_related_holons(
        CoreRelationshipTypeName::HasRelationshipReference,
        vec![HolonReference::Transient(relationship_reference)],
    )?;

    Ok(())
}
