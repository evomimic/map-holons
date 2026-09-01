//! Generation of the distribution-ready Core Schema bootstrap bundle.
//!
//! Canonical loader imports remain projections of TDL and must not be edited.
//! This module derives a separate bootstrap bundle by adding the authored
//! CoreSchemaSpace stewardship relationship to every selected Core holon.

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const CORE_SCHEMA_RELEASE: &str = "MAP Core Schema-v0.0.7";
const CORE_SCHEMA_SPACE_KEY: &str = "MAP.CoreSchemaSpace";
const CORE_SCHEMA_BOOTSTRAP_FILE: &str = "core-schema-bootstrap.json";

#[derive(Debug, Serialize)]
struct BootstrapManifest {
    release_identity: &'static str,
    core_schema_key: &'static str,
    core_schema_space_key: &'static str,
    imports: Vec<BootstrapImport>,
    required_holons: Vec<RequiredHolon>,
}

#[derive(Debug, Serialize)]
struct BootstrapImport {
    path: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct RequiredHolon {
    key: String,
    #[serde(rename = "type")]
    descriptor_key: String,
}

/// Generates an explicit, manifest-selected bootstrap bundle from canonical
/// Core loader imports.
///
/// The output has an `imports/` directory and a `manifest.json`. Every emitted
/// holon declares `OwnedBy -> MAP.CoreSchemaSpace`; normal two-pass Commit is
/// therefore responsible for materializing the inverse `Owns` facts.
pub fn generate_core_schema_bootstrap_bundle(import_dir: &Path, out_dir: &Path) -> Result<()> {
    let mut source_files = fs::read_dir(import_dir)
        .with_context(|| format!("reading Core import directory {}", import_dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;

    source_files.retain(|path| path.extension().is_some_and(|extension| extension == "json"));
    source_files.sort();

    if source_files.is_empty() {
        return Err(anyhow!("no Core JSON imports found in {}", import_dir.display()));
    }

    let bootstrap_path = import_dir.join(CORE_SCHEMA_BOOTSTRAP_FILE);
    if !source_files.contains(&bootstrap_path) {
        return Err(anyhow!(
            "Core bootstrap overlay {} is missing; compile schema-src before generating the bundle",
            bootstrap_path.display()
        ));
    }

    // Keep the overlay last so its staged CoreSchemaSpace is visually and
    // operationally explicit in the bundle. Key references still resolve
    // across the complete staged set rather than depending on file order.
    source_files.retain(|path| path != &bootstrap_path);
    source_files.push(bootstrap_path);

    let imports_dir = out_dir.join("imports");
    fs::create_dir_all(&imports_dir)
        .with_context(|| format!("creating bundle import directory {}", imports_dir.display()))?;

    let mut imports = Vec::with_capacity(source_files.len());
    let mut required_holons = Vec::new();

    for source_path in source_files {
        let source = fs::read_to_string(&source_path)
            .with_context(|| format!("reading generated import {}", source_path.display()))?;
        let mut document: Value = serde_json::from_str(&source)
            .with_context(|| format!("parsing generated import {}", source_path.display()))?;

        let holons = document
            .get_mut("holons")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| anyhow!("generated import {} has no holons array", source_path.display()))?;

        for holon in holons {
            let object = holon.as_object_mut().ok_or_else(|| {
                anyhow!("generated import {} contains a non-object holon", source_path.display())
            })?;
            let key = required_string(object, "key", &source_path)?;
            let descriptor_key = required_string(object, "type", &source_path)?;
            add_core_schema_ownership(object);
            required_holons.push(RequiredHolon { key, descriptor_key });
        }

        let file_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("invalid Core import filename {}", source_path.display()))?;
        let rendered = serde_json::to_vec_pretty(&document)?;
        let output_path = imports_dir.join(file_name);
        fs::write(&output_path, &rendered)
            .with_context(|| format!("writing bootstrap import {}", output_path.display()))?;

        imports.push(BootstrapImport {
            path: format!("imports/{file_name}"),
            sha256: sha256_hex(&rendered),
        });
    }

    required_holons.sort_by(|left, right| left.key.cmp(&right.key));
    let manifest = BootstrapManifest {
        release_identity: CORE_SCHEMA_RELEASE,
        core_schema_key: CORE_SCHEMA_RELEASE,
        core_schema_space_key: CORE_SCHEMA_SPACE_KEY,
        imports,
        required_holons,
    };
    let manifest_path = out_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)
        .with_context(|| format!("writing bootstrap manifest {}", manifest_path.display()))?;

    Ok(())
}

fn required_string(object: &Map<String, Value>, field: &str, source_path: &Path) -> Result<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("generated import {} has no string {field}", source_path.display()))
}

fn add_core_schema_ownership(holon: &mut Map<String, Value>) {
    let relationships = holon
        .entry("relationships")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .expect("generated loader relationship field must be an array");

    let already_owned = relationships.iter().any(|relationship| {
        relationship.get("name").and_then(Value::as_str) == Some("OwnedBy")
    });
    if !already_owned {
        relationships.push(json!({
            "name": "OwnedBy",
            "target": [{"$ref": CORE_SCHEMA_SPACE_KEY}],
        }));
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn adds_owned_by_without_replacing_existing_relationships() {
        let mut holon: Value = serde_json::from_value(json!({
            "key": "Example",
            "type": "Example.HolonType",
            "relationships": [{"name": "Extends", "target": [{"$ref": "Base"}]}],
        }))
        .expect("holon object");

        add_core_schema_ownership(holon.as_object_mut().expect("object"));

        assert_eq!(holon["relationships"].as_array().expect("relationships").len(), 2);
        assert_eq!(
            holon["relationships"][1]["target"][0]["$ref"],
            Value::String(CORE_SCHEMA_SPACE_KEY.to_string())
        );
    }

    #[test]
    fn generates_manifest_and_overlayed_imports() -> Result<()> {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!("map-schema-bootstrap-{suffix}"));
        let imports = root.join("imports-source");
        let output = root.join("bundle");
        fs::create_dir_all(&imports)?;
        fs::write(
            imports.join("root.json"),
            r#"{"holons":[{"key":"MAP Core Schema-v0.0.7","type":"Schema.HolonType"}]}"#,
        )?;
        fs::write(
            imports.join(CORE_SCHEMA_BOOTSTRAP_FILE),
            r#"{"holons":[{"key":"MAP.CoreSchemaSpace","type":"HolonSpace.HolonType","relationships":[{"name":"OwnedBy","target":[{"$ref":"MAP.CoreSchemaSpace"}]}]}]}"#,
        )?;

        generate_core_schema_bootstrap_bundle(&imports, &output)?;

        let manifest: Value = serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
        assert_eq!(manifest["core_schema_space_key"], CORE_SCHEMA_SPACE_KEY);
        assert_eq!(manifest["imports"].as_array().expect("imports").len(), 2);
        let root_import: Value =
            serde_json::from_slice(&fs::read(output.join("imports/root.json"))?)?;
        assert_eq!(
            root_import["holons"][0]["relationships"][0]["target"][0]["$ref"],
            CORE_SCHEMA_SPACE_KEY
        );

        fs::remove_dir_all(root)?;
        Ok(())
    }
}
