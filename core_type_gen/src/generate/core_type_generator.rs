//! Universal generator for core type definitions.
//!
//! This module provides a generalized function, `generate_enum_and_json`, which processes
//! any core type YAML file and produces:
//!
//! 1. A Rust enum definition for use in `type_names`
//! 2. A set of JSON spec files, one per variant, for use as inputs to type definers
//!
//! The function is parameterized over the type of YAML input via the `ParseTypeKind` trait,
//! and uses closures to extract variant names and entries for serialization.
//!
//! This allows a single generator implementation to work across all type kinds, including:
//! - HolonTypes
//! - PropertyTypes
//! - RelationshipTypes
//! - EnumTypes
//!
//! By standardizing the pipeline (YAML → Rust Enum + JSON Specs), this module helps
//! enforce consistency and reduces boilerplate across core type loading.
use crate::parse::type_kind_parser::ParseTypeKind;
use serde::Serialize;
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};

pub fn generate_enum_and_json<T>(
    yaml_path: &Path,
    enum_out_path: &Path,
    json_out_dir: &Path,
    enum_name_prefix: &str,
    get_variant_names: impl Fn(&T) -> Vec<String>,
) -> Result<(), String>
where
    T: ParseTypeKind,
{
    let parsed = T::parse_yaml(yaml_path)?;

    let variant_names = get_variant_names(&parsed);

    crate::generate::enum_template::generate_enum_from_template(
        &format!("{}Name", enum_name_prefix),
        &variant_names,
        enum_out_path,
    )
    .map_err(|e| format!("Enum generation error: {e}"))?;

    fs::create_dir_all(json_out_dir).map_err(|e| format!("Failed to create JSON dir: {e}"))?;

    for (name, item) in parsed.type_spec_items() {
        let json = serde_json::to_string_pretty(item)
            .map_err(|e| format!("Serialization failed for {name}: {e}"))?;

        fs::write(json_out_dir.join(format!("{name}.json")), json)
            .map_err(|e| format!("Failed to write {name}.json: {e}"))?;
    }

    println!("✅ Wrote enum and JSON for {}", T::type_kind_name());
    Ok(())
}
