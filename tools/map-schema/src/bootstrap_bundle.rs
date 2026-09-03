//! Generation of the distribution-ready Core Schema bootstrap bundle.
//!
//! Canonical loader imports remain projections of TDL and must not be edited.
//! This module derives a separate bootstrap bundle by adding the authored
//! CoreSchemaSpace stewardship relationship to every selected Core holon.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path},
};

const CORE_SCHEMA_SPACE_KEY: &str = "MAP.CoreSchemaSpace";
const CORE_SCHEMA_BOOTSTRAP_FILE: &str = "core-schema-bootstrap.json";

#[derive(Debug, Serialize)]
struct BootstrapManifest {
    release_identity: String,
    core_schema_key: String,
    core_schema_space_key: String,
    packages: Vec<BootstrapPackage>,
    imports: Vec<BootstrapImport>,
    required_holons: Vec<RequiredHolon>,
}

#[derive(Debug, Serialize)]
struct BootstrapImport {
    path: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct BootstrapPackage {
    name: String,
    import_directory: String,
}

#[derive(Debug, Deserialize)]
struct BootstrapSourceManifest {
    release_identity: String,
    core_schema_key: String,
    core_schema_space_key: String,
    packages: Vec<BootstrapSourcePackage>,
}

#[derive(Debug, Deserialize)]
struct BootstrapSourcePackage {
    name: String,
    import_directory: String,
}

#[derive(Debug, Serialize)]
struct RequiredHolon {
    key: String,
    #[serde(rename = "type")]
    descriptor_key: String,
}

/// Generates an explicit, manifest-selected bootstrap bundle from canonical
/// operational schema loader imports.
///
/// The output has an `imports/` directory and a `manifest.json`. Every emitted
/// holon declares `OwnedBy -> MAP.CoreSchemaSpace`; normal two-pass Commit is
/// therefore responsible for materializing the inverse `Owns` facts.
pub fn generate_core_schema_bootstrap_bundle(
    import_root: &Path,
    source_manifest_path: &Path,
    out_dir: &Path,
) -> Result<()> {
    let source_manifest: BootstrapSourceManifest =
        serde_json::from_slice(&fs::read(source_manifest_path).with_context(|| {
            format!("reading bootstrap source manifest {}", source_manifest_path.display())
        })?)
        .with_context(|| {
            format!("parsing bootstrap source manifest {}", source_manifest_path.display())
        })?;

    validate_source_manifest(&source_manifest, source_manifest_path)?;

    let mut source_files = Vec::new();
    let mut packages = Vec::with_capacity(source_manifest.packages.len());
    for package in &source_manifest.packages {
        let package_directory = import_root.join(&package.import_directory);
        let mut package_files = fs::read_dir(&package_directory)
            .with_context(|| {
                format!(
                    "reading bootstrap package {} at {}",
                    package.name,
                    package_directory.display()
                )
            })?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        package_files.retain(|path| path.extension().is_some_and(|extension| extension == "json"));
        package_files.sort();
        if package_files.is_empty() {
            return Err(anyhow!(
                "bootstrap package {} has no JSON imports in {}",
                package.name,
                package_directory.display()
            ));
        }
        source_files.extend(package_files);
        packages.push(BootstrapPackage {
            name: package.name.clone(),
            import_directory: package.import_directory.clone(),
        });
    }

    let bootstrap_path = import_root.join("core").join(CORE_SCHEMA_BOOTSTRAP_FILE);
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
    if imports_dir.exists() {
        fs::remove_dir_all(&imports_dir).with_context(|| {
            format!("removing stale bootstrap imports at {}", imports_dir.display())
        })?;
    }
    fs::create_dir_all(&imports_dir)
        .with_context(|| format!("creating bundle import directory {}", imports_dir.display()))?;

    let mut imports = Vec::with_capacity(source_files.len());
    let mut required_holons = Vec::new();

    for source_path in source_files {
        let source = fs::read_to_string(&source_path)
            .with_context(|| format!("reading generated import {}", source_path.display()))?;
        let mut document: Value = serde_json::from_str(&source)
            .with_context(|| format!("parsing generated import {}", source_path.display()))?;

        let holons = document.get_mut("holons").and_then(Value::as_array_mut).ok_or_else(|| {
            anyhow!("generated import {} has no holons array", source_path.display())
        })?;

        for holon in holons {
            let object = holon.as_object_mut().ok_or_else(|| {
                anyhow!("generated import {} contains a non-object holon", source_path.display())
            })?;
            let key = required_string(object, "key", &source_path)?;
            let descriptor_key = required_string(object, "type", &source_path)?;
            add_core_schema_ownership(object);
            required_holons.push(RequiredHolon { key, descriptor_key });
        }

        let relative_source_path = source_path.strip_prefix(import_root).map_err(|_| {
            anyhow!(
                "bootstrap import {} is outside import root {}",
                source_path.display(),
                import_root.display()
            )
        })?;
        let relative_path = relative_source_path.to_string_lossy().replace('\\', "/");
        let rendered = serde_json::to_vec_pretty(&document)?;
        let output_path = imports_dir.join(relative_source_path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("creating bootstrap import directory {}", parent.display())
            })?;
        }
        fs::write(&output_path, &rendered)
            .with_context(|| format!("writing bootstrap import {}", output_path.display()))?;

        imports.push(BootstrapImport {
            path: format!("imports/{relative_path}"),
            sha256: sha256_hex(&rendered),
        });
    }

    required_holons.sort_by(|left, right| left.key.cmp(&right.key));
    let manifest = BootstrapManifest {
        release_identity: source_manifest.release_identity,
        core_schema_key: source_manifest.core_schema_key,
        core_schema_space_key: source_manifest.core_schema_space_key,
        packages,
        imports,
        required_holons,
    };
    let manifest_path = out_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)
        .with_context(|| format!("writing bootstrap manifest {}", manifest_path.display()))?;

    Ok(())
}

fn validate_source_manifest(
    manifest: &BootstrapSourceManifest,
    source_manifest_path: &Path,
) -> Result<()> {
    if manifest.release_identity.is_empty()
        || manifest.core_schema_key.is_empty()
        || manifest.core_schema_space_key != CORE_SCHEMA_SPACE_KEY
        || manifest.packages.is_empty()
        || manifest.packages[0].name != "core"
        || manifest.packages[0].import_directory != "core"
    {
        return Err(anyhow!(
            "bootstrap source manifest {} is incomplete or incompatible",
            source_manifest_path.display()
        ));
    }

    let mut seen_names = std::collections::BTreeSet::new();
    let mut seen_directories = std::collections::BTreeSet::new();
    for package in &manifest.packages {
        let path = Path::new(&package.import_directory);
        if package.name.is_empty()
            || package.import_directory.is_empty()
            || path.is_absolute()
            || path.components().any(|component| !matches!(component, Component::Normal(_)))
            || !seen_names.insert(&package.name)
            || !seen_directories.insert(&package.import_directory)
        {
            return Err(anyhow!(
                "bootstrap source manifest {} contains an invalid or duplicate package selection",
                source_manifest_path.display()
            ));
        }
    }

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

    let already_owned = relationships
        .iter()
        .any(|relationship| relationship.get("name").and_then(Value::as_str) == Some("OwnedBy"));
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
    fn operational_source_manifest_selects_only_required_platform_packages() -> Result<()> {
        let manifest: BootstrapSourceManifest = serde_json::from_str(include_str!(
            "../../../schema-src/core-schema-bootstrap-manifest.json"
        ))?;
        let selected_packages: Vec<&str> =
            manifest.packages.iter().map(|package| package.name.as_str()).collect();

        assert_eq!(
            selected_packages,
            vec!["core", "validation", "dance", "commands", "query", "query-dance", "dahn"]
        );
        assert!(!selected_packages.contains(&"test"));
        assert!(!selected_packages.contains(&"space-navigator"));
        Ok(())
    }

    #[test]
    fn generates_manifest_and_overlayed_imports() -> Result<()> {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!("map-schema-bootstrap-{suffix}"));
        let imports = root.join("imports-source");
        let core_imports = imports.join("core");
        let output = root.join("bundle");
        let source_manifest = root.join("bootstrap-manifest.json");
        fs::create_dir_all(&core_imports)?;
        fs::write(
            core_imports.join("root.json"),
            r#"{"holons":[{"key":"MAP Core Schema-v0.0.7","type":"Schema.HolonType"}]}"#,
        )?;
        fs::write(
            core_imports.join(CORE_SCHEMA_BOOTSTRAP_FILE),
            r#"{"holons":[{"key":"MAP.CoreSchemaSpace","type":"HolonSpace.HolonType","relationships":[{"name":"OwnedBy","target":[{"$ref":"MAP.CoreSchemaSpace"}]}]}]}"#,
        )?;
        fs::write(
            &source_manifest,
            r#"{"release_identity":"MAP Core Schema-v0.0.7","core_schema_key":"MAP Core Schema-v0.0.7","core_schema_space_key":"MAP.CoreSchemaSpace","packages":[{"name":"core","import_directory":"core"}]}"#,
        )?;

        generate_core_schema_bootstrap_bundle(&imports, &source_manifest, &output)?;

        let manifest: Value = serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
        assert_eq!(manifest["core_schema_space_key"], CORE_SCHEMA_SPACE_KEY);
        assert_eq!(manifest["packages"][0]["name"], "core");
        assert_eq!(manifest["imports"].as_array().expect("imports").len(), 2);
        let root_import: Value =
            serde_json::from_slice(&fs::read(output.join("imports/core/root.json"))?)?;
        assert_eq!(
            root_import["holons"][0]["relationships"][0]["target"][0]["$ref"],
            CORE_SCHEMA_SPACE_KEY
        );

        fs::remove_dir_all(root)?;
        Ok(())
    }
}
