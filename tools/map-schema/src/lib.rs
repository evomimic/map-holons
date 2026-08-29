//! Tools for translating MAP loader JSON imports into TDL and loader JSON.
//!
//! This crate is the native tooling layer around the MAP schema corpus. It reads the
//! JSON import format used by the loader, projects those files into deterministic
//! loader facts, and renders TDL that can be compiled back into loader JSON.
//! The decompile path intentionally works over a corpus rather than isolated files
//! so schema dependencies and cross-file references can be resolved consistently.

use anyhow::{anyhow, Context, Result};
pub mod diagnostics;
/// TDL parser, checker, and compiler entry points.
pub mod tdl_compiler;

use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

const INDENT: &str = "  ";

#[derive(Debug, Clone, Deserialize)]
struct ImportFile {
    #[serde(default)]
    meta: serde_json::Map<String, Value>,
    holons: Vec<HolonRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct HolonRecord {
    key: String,
    #[serde(rename = "type")]
    descriptor_type: String,
    #[serde(default)]
    properties: serde_json::Map<String, Value>,
    #[serde(default)]
    relationships: Vec<RelationshipRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct RelationshipRecord {
    name: String,
    target: Value,
}

#[derive(Debug, Clone)]
struct ParsedFile {
    relative_path: PathBuf,
    schema_name: String,
    import: ImportFile,
}

/// Decompiles JSON import files into TDL files under `out_dir`.
///
/// Each input may be either a single `.json` file or a directory tree containing
/// JSON import files. Directory inputs preserve relative paths in the output tree,
/// replacing each `.json` extension with `.tdl`. The returned paths are the TDL
/// files written during this run.
pub fn decompile_inputs(inputs: &[PathBuf], out_dir: &Path) -> Result<Vec<PathBuf>> {
    let project = parse_json_inputs_to_loader_fact_project(inputs)?;
    let mut written = Vec::new();

    for file in &project.files {
        let output = out_dir.join(file.relative_path.with_extension("tdl"));
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }

        let contents = render_loader_fact_file(file)?;
        fs::write(&output, contents)
            .with_context(|| format!("writing decompiled TDL to {}", output.display()))?;
        written.push(output);
    }

    Ok(written)
}

/// Decompiles one JSON import document provided as a raw string.
///
/// This helper is intended for tests and embeddings that already have the import
/// contents in memory. Because it receives a single document, dependency names are
/// inferred only from that document and cannot be resolved through neighboring
/// files the way `decompile_inputs` can.
pub fn decompile_input_string(raw: &str, source_name: impl Into<PathBuf>) -> Result<String> {
    let source_name = source_name.into();
    let parsed = parse_import_file_contents(raw, &source_name, source_name.clone())?;
    let project = loader_fact_project_from_parsed(vec![parsed])?;
    let file = project
        .files
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no JSON import document was projected"))?;
    render_loader_fact_file(&file)
}

#[derive(Debug, Clone)]
pub struct RoundTripReport {
    pub decompiled_files: Vec<PathBuf>,
    pub compiled_files: Vec<PathBuf>,
}

pub fn roundtrip_json_inputs(
    inputs: &[PathBuf],
    tdl_out_dir: &Path,
    json_out_dir: &Path,
) -> Result<RoundTripReport> {
    let before = loader_fact_signature_for_inputs(inputs)?;
    let decompiled_files = decompile_inputs(inputs, tdl_out_dir)?;
    let compiled_files =
        crate::tdl_compiler::compile_inputs(&[tdl_out_dir.to_path_buf()], json_out_dir)?;
    let after = loader_fact_signature_for_inputs(&[json_out_dir.to_path_buf()])?;

    if before != after {
        return Err(anyhow!(
            "round-trip LoaderRefRep signatures differ\n{}",
            loader_fact_signature_diff(&before, &after)
        ));
    }

    Ok(RoundTripReport { decompiled_files, compiled_files })
}

#[derive(Debug, Clone)]
struct DiscoveredFile {
    source_path: PathBuf,
    relative_path: PathBuf,
}

fn collect_input_files(inputs: &[PathBuf]) -> Result<Vec<DiscoveredFile>> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_dir() {
            collect_json_files(input, input, &mut files)?;
        } else if input.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let relative_path =
                input.file_name().map(PathBuf::from).unwrap_or_else(|| input.clone());
            files.push(DiscoveredFile { source_path: input.clone(), relative_path });
        }
    }
    ensure_unique_relative_paths(&files)?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn collect_json_files(root: &Path, current: &Path, files: &mut Vec<DiscoveredFile>) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("reading input directory {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(root, &path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(DiscoveredFile {
                source_path: path.clone(),
                relative_path: path.strip_prefix(root).map(Path::to_path_buf).unwrap_or_else(
                    |_| path.file_name().map(PathBuf::from).unwrap_or_else(|| path.clone()),
                ),
            });
        }
    }
    Ok(())
}

fn parse_files(discovered: &[DiscoveredFile]) -> Result<Vec<ParsedFile>> {
    let mut parsed = Vec::with_capacity(discovered.len());

    for discovered_file in discovered {
        let path = &discovered_file.source_path;
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading JSON import file {}", path.display()))?;
        parsed.push(parse_import_file_contents(&raw, path, discovered_file.relative_path.clone())?);
    }

    Ok(parsed)
}

fn parse_import_file_contents(
    raw: &str,
    source_path: &Path,
    relative_path: PathBuf,
) -> Result<ParsedFile> {
    let import: ImportFile = serde_json::from_str(raw)
        .with_context(|| format!("parsing JSON import file {}", source_path.display()))?;
    let schema_name = infer_schema_name(&import)
        .with_context(|| format!("inferring schema name for {}", source_path.display()))?;
    Ok(ParsedFile { relative_path, schema_name, import })
}

#[derive(Debug, Clone)]
struct LoaderFactProject {
    files: Vec<LoaderFactFile>,
}

#[derive(Debug, Clone)]
struct LoaderFactFile {
    relative_path: PathBuf,
    meta: serde_json::Map<String, Value>,
    schema_key: String,
    schema_holon: Option<LoaderFactHolon>,
    emits_schema_holon: bool,
    holons: Vec<LoaderFactHolon>,
}

#[derive(Debug, Clone)]
struct LoaderFactHolon {
    key: String,
    descriptor_type: String,
    properties: BTreeMap<String, Value>,
    relationships: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LoaderFactSignature {
    files: Vec<LoaderFactFileSignature>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LoaderFactFileSignature {
    relative_path: String,
    meta: String,
    holons: Vec<LoaderFactHolonSignature>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LoaderFactHolonSignature {
    key: String,
    descriptor_type: String,
    properties: Vec<(String, String)>,
    relationships: Vec<(String, Vec<String>)>,
}

fn parse_json_inputs_to_loader_fact_project(inputs: &[PathBuf]) -> Result<LoaderFactProject> {
    let files = collect_input_files(inputs)?;
    let parsed = parse_files(&files)?;
    loader_fact_project_from_parsed(parsed)
}

fn loader_fact_project_from_parsed(parsed: Vec<ParsedFile>) -> Result<LoaderFactProject> {
    let schema_owner_paths = json_schema_owner_paths(&parsed);
    let mut files = Vec::with_capacity(parsed.len());

    for parsed_file in parsed {
        let schema_key = parsed_file.schema_name.clone();
        let emits_schema_holon = schema_owner_paths
            .get(&schema_key)
            .map(|owner_path| owner_path == &parsed_file.relative_path)
            .unwrap_or(true);
        let mut schema_holon = None;
        let mut holons = Vec::new();

        for holon in parsed_file.import.holons {
            let projected = loader_fact_holon_from_record(holon);
            if projected.descriptor_type == "Schema.HolonType" {
                schema_holon = Some(projected);
            } else {
                holons.push(projected);
            }
        }

        files.push(LoaderFactFile {
            relative_path: parsed_file.relative_path,
            meta: parsed_file.import.meta.clone(),
            schema_key,
            schema_holon,
            emits_schema_holon,
            holons,
        });
    }

    Ok(LoaderFactProject { files })
}

fn json_schema_owner_paths(parsed: &[ParsedFile]) -> HashMap<String, PathBuf> {
    let mut owner_paths = HashMap::<String, PathBuf>::new();
    for file in parsed {
        let schema_name = file.schema_name.clone();
        let has_schema_holon =
            file.import.holons.iter().any(|holon| holon.descriptor_type == "Schema.HolonType");
        if !has_schema_holon {
            continue;
        }
        let existing_owner = owner_paths.get(&schema_name);
        let should_replace = existing_owner
            .map(|path| is_preferred_json_schema_owner(&file.relative_path, path))
            .unwrap_or(true);
        if should_replace {
            owner_paths.insert(schema_name, file.relative_path.clone());
        }
    }
    owner_paths
}

fn is_preferred_json_schema_owner(candidate: &Path, current: &Path) -> bool {
    let candidate_name = candidate.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
    let current_name = current.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
    candidate_name.contains("root") && !current_name.contains("root")
}

fn loader_fact_holon_from_record(holon: HolonRecord) -> LoaderFactHolon {
    let mut relationships = BTreeMap::<String, Vec<String>>::new();
    for relationship in holon.relationships {
        relationships
            .entry(relationship.name)
            .or_default()
            .extend(target_strings(&relationship.target));
    }

    LoaderFactHolon {
        key: holon.key,
        descriptor_type: holon.descriptor_type,
        properties: holon
            .properties
            .into_iter()
            .map(|(name, value)| (canonical_loader_fact_property_name(&name), value))
            .collect(),
        relationships,
    }
}

fn canonical_loader_fact_property_name(name: &str) -> String {
    match name {
        "schema_name" => "SchemaName",
        "type_name" => "TypeName",
        "type_name_plural" => "TypeNamePlural",
        "display_name" => "DisplayName",
        "display_name_plural" => "DisplayNamePlural",
        "description" => "Description",
        "is_abstract_type" => "IsAbstractType",
        "allows_additional_properties" => "AllowsAdditionalProperties",
        "allows_additional_relationships" => "AllowsAdditionalRelationships",
        "is_definitional" => "IsDefinitional",
        "is_ordered" => "IsOrdered",
        "allows_duplicates" => "AllowsDuplicates",
        "deletion_semantic" => "DeletionSemantic",
        other => other,
    }
    .to_string()
}

fn loader_fact_signature_for_inputs(inputs: &[PathBuf]) -> Result<LoaderFactSignature> {
    let project = parse_json_inputs_to_loader_fact_project(inputs)?;
    Ok(loader_fact_signature(&project))
}

fn loader_fact_signature(project: &LoaderFactProject) -> LoaderFactSignature {
    let files = project
        .files
        .iter()
        .map(|file| {
            let mut holons = Vec::new();
            if file.emits_schema_holon {
                if let Some(schema_holon) = &file.schema_holon {
                    holons.push(loader_fact_holon_signature(schema_holon));
                }
            }
            holons.extend(file.holons.iter().map(loader_fact_holon_signature));
            LoaderFactFileSignature {
                relative_path: normalize_relative_path(&file.relative_path),
                meta: canonical_value_string(&Value::Object(file.meta.clone())),
                holons,
            }
        })
        .collect();
    LoaderFactSignature { files }
}

fn loader_fact_holon_signature(holon: &LoaderFactHolon) -> LoaderFactHolonSignature {
    LoaderFactHolonSignature {
        key: holon.key.clone(),
        descriptor_type: holon.descriptor_type.clone(),
        properties: ordered_loader_fact_properties(&holon.properties)
            .into_iter()
            .map(|(name, value)| (name, canonical_value_string(&value)))
            .collect(),
        relationships: ordered_loader_fact_relationships(&holon.relationships),
    }
}

fn canonical_value_string(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn loader_fact_signature_diff(
    expected: &LoaderFactSignature,
    actual: &LoaderFactSignature,
) -> String {
    let expected_text = format!("{expected:#?}");
    let actual_text = format!("{actual:#?}");
    format!("expected:\n{expected_text}\nactual:\n{actual_text}")
}

fn render_loader_fact_file(file: &LoaderFactFile) -> Result<String> {
    let mut out = String::new();
    let variant_targets = loader_fact_variant_targets(file);
    if !file.meta.is_empty() {
        out.push_str("meta {\n");
        for (name, value) in &file.meta {
            out.push_str(&format!("{}{}: {}\n", INDENT, name, canonical_value_string(value)));
        }
        out.push_str("}\n\n");
    }
    render_loader_fact_schema_decl(&mut out, file)?;

    for holon in &file.holons {
        out.push('\n');
        render_loader_fact_holon(&mut out, holon, &variant_targets)?;
    }

    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    Ok(out)
}

fn loader_fact_variant_targets(file: &LoaderFactFile) -> HashSet<String> {
    file.holons
        .iter()
        .filter_map(|holon| holon.relationships.get("Variants"))
        .flatten()
        .cloned()
        .collect()
}

fn render_loader_fact_schema_decl(out: &mut String, file: &LoaderFactFile) -> Result<()> {
    let Some(schema_holon) = file.schema_holon.as_ref().filter(|_| file.emits_schema_holon) else {
        out.push_str(&format!("schema {}\n", render_reference_token(&file.schema_key)));
        return Ok(());
    };

    let depends_on = schema_holon.relationships.get("DependsOn").cloned().unwrap_or_default();
    let header_lines = header_lines_from_properties(&schema_holon.properties);
    let literal_properties = schema_literal_properties(&schema_holon.properties);
    let literal_relationships = schema_literal_relationships(&schema_holon.relationships);
    let has_body = !depends_on.is_empty()
        || !header_lines.is_empty()
        || !literal_properties.is_empty()
        || !literal_relationships.is_empty();

    if !has_body {
        out.push_str(&format!("schema {}\n", render_reference_token(&file.schema_key)));
        return Ok(());
    }

    out.push_str(&format!("schema {} {{\n", render_reference_token(&file.schema_key)));
    for dependency in depends_on {
        out.push_str(&format!("{}depends_on {}\n", INDENT, render_reference_token(&dependency)));
    }
    render_header_lines(out, 1, &header_lines);
    render_properties_block(out, 1, &literal_properties);
    render_relationships_block(out, 1, &literal_relationships);
    out.push_str("}\n");
    Ok(())
}

fn render_loader_fact_holon(
    out: &mut String,
    holon: &LoaderFactHolon,
    variant_targets: &HashSet<String>,
) -> Result<()> {
    let component_of_targets = holon.relationships.get("ComponentOf").cloned().unwrap_or_default();
    let type_name = holon.properties.get("TypeName").and_then(Value::as_str);
    let is_relationship_key = relationship_label_from_key(&holon.key).is_some();
    let use_variant_form = variant_targets.contains(&holon.key)
        && component_of_targets.len() == 1
        && !is_relationship_key
        && type_name
            .map(|value| value == local_loader_fact_variant_name(&holon.key))
            .unwrap_or(true);
    let type_name_matches_holon_form = type_name
        .map(|type_name| type_name == local_loader_fact_type_name(&holon.key))
        .unwrap_or(true);
    let use_holon_form = !use_variant_form
        && !is_relationship_key
        && component_of_targets.len() == 1
        && type_name_matches_holon_form;
    let declaration = if use_variant_form {
        "variant"
    } else if use_holon_form {
        "holon"
    } else {
        "instance"
    };
    let use_descriptor_form = use_variant_form || use_holon_form;

    out.push_str(&format!("{} {} {{\n", declaration, render_reference_token(&holon.key)));
    out.push_str(&format!("{}type {}\n", INDENT, render_reference_token(&holon.descriptor_type)));
    let rule_of = (declaration == "instance")
        .then(|| single_relationship_target(&holon.relationships, "RuleOf"))
        .flatten();
    if let Some(rule_of) = &rule_of {
        out.push_str(&format!("{}rule_of {}\n", INDENT, render_reference_token(rule_of)));
    }
    let cardinality_shorthand = (declaration == "instance"
        && holon.descriptor_type == "CardinalityConstraint.ConstraintType")
        .then(|| {
            holon.properties.get("Minimum").and_then(Value::as_i64).map(|minimum| {
                let maximum = holon
                    .properties
                    .get("Maximum")
                    .and_then(Value::as_i64)
                    .map_or_else(|| "*".to_string(), |maximum| maximum.to_string());
                (minimum, maximum)
            })
        })
        .flatten();
    if let Some((minimum, maximum)) = &cardinality_shorthand {
        out.push_str(&format!("{}cardinality {}..{}\n", INDENT, minimum, maximum));
    }
    if use_descriptor_form {
        if let Some(extends) = single_relationship_target(&holon.relationships, "Extends") {
            out.push_str(&format!("{}extends {}\n", INDENT, render_reference_token(&extends)));
        }
        if let Some(value_type) = single_relationship_target(&holon.relationships, "ValueType") {
            out.push_str(&format!("{}value {}\n", INDENT, render_reference_token(&value_type)));
        }
        if let Some(key_rule) = single_relationship_target(&holon.relationships, "InstanceKeyRule")
        {
            out.push_str(&format!(
                "{}instance_keyrule {}\n",
                INDENT,
                render_reference_token(&key_rule)
            ));
        }
    }
    let properties = ordered_loader_fact_properties(&holon.properties)
        .into_iter()
        .filter(|(name, _)| {
            !(use_descriptor_form && name == "TypeName")
                && !(cardinality_shorthand.is_some()
                    && matches!(name.as_str(), "Minimum" | "Maximum"))
        })
        .collect::<Vec<_>>();
    let relationships = ordered_loader_fact_relationships(&holon.relationships)
        .into_iter()
        .filter_map(|(name, targets)| {
            if use_descriptor_form && name == "Extends" && targets.len() == 1 {
                return None;
            }
            if use_descriptor_form && name == "ValueType" && targets.len() == 1 {
                return None;
            }
            if use_descriptor_form && name == "InstanceKeyRule" && targets.len() == 1 {
                return None;
            }
            if rule_of.is_some() && name == "RuleOf" && targets.len() == 1 {
                return None;
            }
            if use_descriptor_form && name == "ComponentOf" && targets == component_of_targets {
                return None;
            }
            Some((name, targets))
        })
        .collect::<Vec<_>>();
    render_properties_block(out, 1, &properties);
    render_relationships_block(out, 1, &relationships);
    out.push_str("}\n");
    Ok(())
}

fn single_relationship_target(
    relationships: &BTreeMap<String, Vec<String>>,
    name: &str,
) -> Option<String> {
    let targets = relationships.get(name)?;
    if targets.len() == 1 {
        Some(targets[0].clone())
    } else {
        None
    }
}

fn local_loader_fact_type_name(key: &str) -> &str {
    if let Some(name) = relationship_label_from_key(key) {
        return name;
    }
    key.split('.').next().unwrap_or(key)
}

fn local_loader_fact_variant_name(key: &str) -> &str {
    key.rsplit_once('.').map(|(_, local)| local).unwrap_or(key)
}

fn relationship_label_from_key(key: &str) -> Option<&str> {
    let (_, rest) = key.split_once(")-[")?;
    let (name, _) = rest.split_once("]->(")?;
    Some(name)
}

fn header_lines_from_properties(
    properties: &BTreeMap<String, Value>,
) -> Vec<(&'static str, String)> {
    [
        ("description", "Description"),
        ("display_name", "DisplayName"),
        ("display_plural", "DisplayNamePlural"),
        ("plural", "TypeNamePlural"),
    ]
    .into_iter()
    .filter_map(|(header_name, property_name)| {
        properties
            .get(property_name)
            .and_then(Value::as_str)
            .map(|value| (header_name, value.to_string()))
    })
    .collect()
}

fn schema_literal_properties(properties: &BTreeMap<String, Value>) -> Vec<(String, Value)> {
    ordered_loader_fact_properties(properties)
        .into_iter()
        .filter(|(name, _)| {
            !matches!(
                name.as_str(),
                "SchemaName"
                    | "Description"
                    | "DisplayName"
                    | "DisplayNamePlural"
                    | "TypeNamePlural"
            )
        })
        .collect()
}

fn schema_literal_relationships(
    relationships: &BTreeMap<String, Vec<String>>,
) -> Vec<(String, Vec<String>)> {
    ordered_loader_fact_relationships(relationships)
        .into_iter()
        .filter(|(name, _)| name != "DependsOn")
        .collect()
}

fn render_header_lines(out: &mut String, indent: usize, lines: &[(&'static str, String)]) {
    if lines.is_empty() {
        return;
    }
    out.push_str(&format!("{}header {{\n", INDENT.repeat(indent)));
    for (name, value) in lines {
        out.push_str(&format!("{}{}: {}\n", INDENT.repeat(indent + 1), name, json_literal(value)));
    }
    out.push_str(&format!("{}}}\n", INDENT.repeat(indent)));
}

fn render_properties_block(out: &mut String, indent: usize, properties: &[(String, Value)]) {
    if properties.is_empty() {
        return;
    }
    out.push_str(&format!("{}properties {{\n", INDENT.repeat(indent)));
    for (name, value) in properties {
        out.push_str(&format!(
            "{}{}: {}\n",
            INDENT.repeat(indent + 1),
            name,
            canonical_value_string(value)
        ));
    }
    out.push_str(&format!("{}}}\n", INDENT.repeat(indent)));
}

fn render_relationships_block(
    out: &mut String,
    indent: usize,
    relationships: &[(String, Vec<String>)],
) {
    if relationships.is_empty() {
        return;
    }
    out.push_str(&format!("{}relationships {{\n", INDENT.repeat(indent)));
    for (name, targets) in relationships {
        out.push_str(&format!(
            "{}{} -> {}\n",
            INDENT.repeat(indent + 1),
            name,
            render_relationship_targets(targets)
        ));
    }
    out.push_str(&format!("{}}}\n", INDENT.repeat(indent)));
}

fn ordered_loader_fact_properties(properties: &BTreeMap<String, Value>) -> Vec<(String, Value)> {
    let preferred = [
        "SchemaName",
        "TypeName",
        "TypeNamePlural",
        "DisplayName",
        "DisplayNamePlural",
        "Description",
        "IsAbstractType",
        "DefinesInstanceTypeKind",
        "IsDefinitional",
        "IsOrdered",
        "AllowsDuplicates",
        "Minimum",
        "Maximum",
        "DeletionSemantic",
        "IsValueRequired",
        "DefaultValue",
        "AllowsAdditionalProperties",
        "AllowsAdditionalRelationships",
    ];
    order_loader_fact_entries(properties, &preferred)
}

fn ordered_loader_fact_relationships(
    relationships: &BTreeMap<String, Vec<String>>,
) -> Vec<(String, Vec<String>)> {
    let preferred = [
        "Extends",
        "ComponentOf",
        "DependsOn",
        "SourceType",
        "TargetType",
        "ValueType",
        "InstanceKeyRule",
        "RuleOf",
        "HasInverse",
        "Variants",
        "InstanceProperties",
        "InstanceRelationships",
        "AffordsCommand",
        "AffordsDance",
        "AffordsOperator",
    ];
    order_loader_fact_entries(relationships, &preferred)
}

fn order_loader_fact_entries<T: Clone>(
    values: &BTreeMap<String, T>,
    preferred: &[&str],
) -> Vec<(String, T)> {
    let mut remaining = values.clone();
    let mut ordered = Vec::with_capacity(remaining.len());
    for name in preferred {
        if let Some(value) = remaining.remove(*name) {
            ordered.push(((*name).to_string(), value));
        }
    }
    ordered.extend(remaining);
    ordered
}

fn render_reference_token(value: &str) -> String {
    json_literal(value)
}

fn ensure_unique_relative_paths(files: &[DiscoveredFile]) -> Result<()> {
    let mut seen = HashMap::<String, PathBuf>::new();
    for file in files {
        let key = normalize_relative_path(&file.relative_path);
        if let Some(existing) = seen.insert(key.clone(), file.source_path.clone()) {
            return Err(anyhow!(
                "duplicate relative input path `{key}` from {} and {}; use a single input root or rename one path",
                existing.display(),
                file.source_path.display()
            ));
        }
    }
    Ok(())
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn infer_schema_name(import: &ImportFile) -> Option<String> {
    import
        .holons
        .iter()
        .find(|holon| holon.descriptor_type == "Schema.HolonType")
        .and_then(schema_name_from_holon)
        .or_else(|| import.holons.iter().find_map(component_of_schema_name))
}

fn schema_name_from_holon(holon: &HolonRecord) -> Option<String> {
    string_property(&holon.properties, "SchemaName")
        .or_else(|| string_property(&holon.properties, "schema_name"))
        .or_else(|| component_of_schema_name(holon))
}

fn component_of_schema_name(holon: &HolonRecord) -> Option<String> {
    relationship_targets(holon, "ComponentOf").into_iter().next()
}

fn render_relationship_targets(targets: &[String]) -> String {
    if targets.len() == 1 {
        json_literal(&targets[0])
    } else {
        let rendered = targets.iter().map(|target| json_literal(target)).collect::<Vec<_>>();
        format!("[{}]", rendered.join(", "))
    }
}

fn relationship_targets(holon: &HolonRecord, name: &str) -> Vec<String> {
    holon
        .relationships
        .iter()
        .filter(|relationship| relationship.name == name)
        .flat_map(|relationship| target_strings(&relationship.target))
        .collect()
}

fn target_strings(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => map
            .get("$ref")
            .and_then(Value::as_str)
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        Value::Array(values) => values.iter().flat_map(target_strings).collect(),
        Value::String(value) => vec![value.clone()],
        _ => Vec::new(),
    }
}

fn string_property(properties: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    properties.get(key).and_then(Value::as_str).map(ToString::to_string)
}

fn json_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{}\"", value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tdl_compiler::compile_inputs;
    use std::{
        env,
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn generated_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("generated")
            .join("json-imports")
    }

    fn sweettests_import_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("sweetests")
            .join("import_files")
    }

    fn temp_out_dir() -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("map-schema-decompile-{nanos}"))
    }

    fn temp_roundtrip_tdl_dir() -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("map-schema-roundtrip-tdl-{nanos}"))
    }

    fn temp_roundtrip_json_dir() -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("map-schema-roundtrip-json-{nanos}"))
    }

    fn temp_domain_json_dir() -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("map-schema-domain-json-{nanos}"))
    }

    fn copy_directory_tree(source: &Path, target: &Path) -> Result<()> {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let path = entry.path();
            let target_path = target.join(entry.file_name());
            if path.is_dir() {
                copy_directory_tree(&path, &target_path)?;
            } else {
                fs::copy(&path, &target_path)?;
            }
        }
        Ok(())
    }

    fn write_json_file(path: &Path, contents: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }

    fn discovered_json_file_count(root: &Path) -> Result<usize> {
        Ok(collect_input_files(&[root.to_path_buf()])?.len())
    }

    #[test]
    fn decompiles_schema_depends_on_relationships_into_tdl_dependencies() -> Result<()> {
        let out_dir = temp_out_dir();
        let source_file =
            sweettests_import_fixture_dir().join("map-test-schema-book-person-inverse.json");
        decompile_inputs(&[source_file], &out_dir)?;

        let tdl = fs::read_to_string(out_dir.join("map-test-schema-book-person-inverse.tdl"))?;

        assert!(tdl.contains(
            "schema \"BookAuthorInverseSchema\" {\n  depends_on \"MAP Core Schema-v0.0.7\""
        ));

        Ok(())
    }

    #[test]
    fn decompiles_single_rule_of_relationship_as_rule_of_clause() -> Result<()> {
        let tdl = decompile_input_string(
            r#"{
  "holons": [
    {
      "key": "Example.Schema",
      "type": "Schema.HolonType",
      "properties": { "SchemaName": "Example.Schema" }
    },
    {
      "key": "DisplayName.StringLengthConstraint",
      "type": "StringLengthConstraint.ConstraintType",
      "properties": { "ConstraintName": "DisplayName", "Minimum": 1 },
      "relationships": [
        { "name": "RuleOf", "target": [{ "$ref": "Example.Schema" }] }
      ]
    }
  ]
}"#,
            "rule-of.json",
        )?;

        assert!(tdl.contains("rule_of \"Example.Schema\""));
        assert!(!tdl.contains("RuleOf ->"));
        Ok(())
    }

    #[test]
    fn roundtrip_json_preserves_generated_core_schema_loader_fact_signature() -> Result<()> {
        let source_dir = generated_fixture_dir();
        let decompiled_tdl_dir = temp_roundtrip_tdl_dir();
        let roundtrip_json_dir = temp_roundtrip_json_dir();
        let expected_json_file_count = discovered_json_file_count(&source_dir)?;

        let decompiled_files = decompile_inputs(&[source_dir.clone()], &decompiled_tdl_dir)?;
        assert_eq!(
            decompiled_files.len(),
            expected_json_file_count,
            "decompile should emit one TDL file per discovered JSON input"
        );

        let roundtrip_files = compile_inputs(&[decompiled_tdl_dir], &roundtrip_json_dir)?;
        assert_eq!(roundtrip_files.len(), expected_json_file_count);
        assert_eq!(
            loader_fact_signature_for_inputs(&[source_dir])?,
            loader_fact_signature_for_inputs(&[roundtrip_json_dir])?
        );

        Ok(())
    }

    #[test]
    fn decompiler_preserves_normalized_relationship_pair_facts() -> Result<()> {
        let out_dir = temp_out_dir();
        decompile_inputs(&[generated_fixture_dir()], &out_dir)?;

        let relationship_types =
            std::fs::read_to_string(out_dir.join("core/relationship-types.tdl"))?;

        assert!(relationship_types
            .contains("instance \"(TypeDescriptor)-[ComponentOf]->(Schema.HolonType)\""));
        assert!(relationship_types
            .contains("HasInverse -> \"(Schema.HolonType)-[Components]->(TypeDescriptor)\""));
        assert!(!relationship_types.contains(
            "HasInverse -> [\"(Schema.HolonType)-[Components]->(TypeDescriptor)\", \"Components\"]"
        ));
        assert!(relationship_types.contains("SourceType -> \"TypeDescriptor\""));
        assert!(relationship_types.contains("TargetType -> \"Schema.HolonType\""));

        Ok(())
    }

    #[test]
    fn decompile_and_compile_arbitrary_json_directory_preserves_nested_paths_and_dependencies(
    ) -> Result<()> {
        let source_dir = temp_domain_json_dir();
        let copied_input_dir = source_dir.join("domain/core-schema");
        copy_directory_tree(&generated_fixture_dir(), &copied_input_dir)?;
        let expected_file_count = discovered_json_file_count(&source_dir)?;

        let decompiled_dir = temp_out_dir();
        let decompiled_files = decompile_inputs(&[source_dir.clone()], &decompiled_dir)?;
        assert_eq!(decompiled_files.len(), expected_file_count);
        assert!(decompiled_files
            .iter()
            .any(|path| { path.to_string_lossy().ends_with("domain/core-schema/core/root.tdl") }));
        assert!(decompiled_files.iter().any(|path| {
            path.to_string_lossy().ends_with("domain/core-schema/dance/schema.tdl")
        }));

        let roundtrip_json_dir = temp_roundtrip_json_dir();
        let roundtrip_files = compile_inputs(&[decompiled_dir], &roundtrip_json_dir)?;
        assert_eq!(roundtrip_files.len(), expected_file_count);
        assert_eq!(
            loader_fact_signature_for_inputs(&[source_dir])?,
            loader_fact_signature_for_inputs(&[roundtrip_json_dir])?
        );

        Ok(())
    }

    #[test]
    fn decompile_rejects_duplicate_relative_paths_across_input_roots() -> Result<()> {
        let root_a = temp_domain_json_dir().join("root-a");
        let root_b = temp_domain_json_dir().join("root-b");
        let out_dir = temp_out_dir();
        let json = r#"{
  "meta": {},
  "holons": [
    {
      "key": "Example Schema-v0.0.1",
      "type": "Schema.HolonType",
      "properties": {
        "schema_name": "Example Schema-v0.0.1"
      }
    }
  ]
}"#;

        write_json_file(&root_a.join("same.json"), json)?;
        write_json_file(&root_b.join("same.json"), json)?;

        let error = decompile_inputs(&[root_a, root_b], &out_dir).expect_err("duplicate paths");
        assert!(error.to_string().contains("duplicate relative input path `same.json`"));

        Ok(())
    }
}
