use crate::diagnostics::{format_diagnostics, Diagnostic};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

type TdlLiteralObject = BTreeMap<String, TdlLiteralValue>;

#[derive(Debug, Clone)]
enum TdlLiteralValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Number(String),
    String(String),
    Array(Vec<TdlLiteralValue>),
    Object(TdlLiteralObject),
}

impl TdlLiteralValue {
    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DescriptorKind {
    HolonType,
    ValueType,
    Enum,
    PropertyType,
    RelationshipType,
    EnumVariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelationshipFlavor {
    Declared,
    Inverse,
}

#[derive(Debug, Clone, Default)]
struct DescriptorHeader {
    description: Option<String>,
    display_name: Option<String>,
    display_name_plural: Option<String>,
    type_name_plural: Option<String>,
}

#[derive(Debug, Clone)]
struct LiteralRelationship {
    name: String,
    targets: Vec<String>,
}

#[derive(Debug, Clone)]
struct DiscoveredFile {
    source_path: PathBuf,
    relative_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ParsedTdlFile {
    relative_path: PathBuf,
    meta: TdlLiteralObject,
    schema: TdlSchema,
    descriptors: Vec<TdlDescriptor>,
}

#[derive(Debug, Clone)]
struct TdlSchema {
    name: String,
    dependencies: Vec<String>,
    literal_properties: TdlLiteralObject,
    literal_relationships: Vec<LiteralRelationship>,
    header: Option<DescriptorHeader>,
    allows_additional_properties: bool,
    allows_additional_relationships: bool,
}

#[derive(Debug, Clone)]
struct TdlDescriptor {
    kind: DescriptorKind,
    name: String,
    header: Option<DescriptorHeader>,
    is_generic_instance: bool,
    is_abstract: bool,
    relationship_flavor: Option<RelationshipFlavor>,
    descriptor_type: Option<String>,
    extends: Option<String>,
    value_type: Option<String>,
    source_type: Option<String>,
    target_type: Option<String>,
    inverse_of: Option<String>,
    has_inverse: Option<String>,
    key_rule: Option<String>,
    min_cardinality: Option<i64>,
    max_cardinality: Option<i64>,
    deletion_semantic: Option<String>,
    is_ordered: bool,
    allows_duplicates: bool,
    allows_additional_properties: bool,
    allows_additional_relationships: bool,
    is_definitional: bool,
    variants: Vec<String>,
    variant_of: Option<String>,
    literal_properties: TdlLiteralObject,
    instance_properties: Vec<String>,
    instance_relationships: Vec<String>,
    literal_relationships: Vec<LiteralRelationship>,
}

pub fn compile_inputs(inputs: &[PathBuf], out_dir: &Path) -> Result<Vec<PathBuf>> {
    let compilation = build_r6_compilation(parse_inputs(inputs)?)?;

    let mut written = Vec::new();
    for file in &compilation.files {
        let output = out_dir.join(file.relative_path.with_extension("json"));
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }

        let contents = file.contents.clone();
        fs::write(&output, contents)
            .with_context(|| format!("writing compiled JSON to {}", output.display()))?;
        written.push(output);
    }

    Ok(written)
}

/// Compiles one TDL document provided as a raw string into loader JSON.
pub fn compile_input_string(raw: &str, source_name: impl Into<PathBuf>) -> Result<String> {
    let source_name = source_name.into();
    let parsed = parse_tdl_file(raw, &source_name)?;
    let compilation = build_r6_compilation(vec![parsed])?;
    let file = compilation
        .files
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no TDL document was compiled"))?;
    Ok(file.contents)
}

pub fn check_inputs(inputs: &[PathBuf]) -> Result<Vec<Diagnostic>> {
    build_r6_compilation(parse_inputs(inputs)?)?;
    Ok(Vec::new())
}

/// Renders the CLI output for `map-schema:check`.
///
/// The current contract is intentionally simple:
/// - `no diagnostics` when the schema set is clean
/// - otherwise, the newline-separated diagnostic stream emitted by
///   `format_diagnostics`
pub fn render_check_output(diagnostics: &[Diagnostic]) -> String {
    if diagnostics.is_empty() {
        "no diagnostics\n".to_string()
    } else {
        format!("{}\n", format_diagnostics(diagnostics))
    }
}

/// Validates one TDL document provided as a raw string.
pub fn check_input_string(raw: &str, source_name: impl Into<PathBuf>) -> Result<Vec<Diagnostic>> {
    let source_name = source_name.into();
    let parsed = parse_tdl_file(raw, &source_name)?;
    build_r6_compilation(vec![parsed])?;
    Ok(Vec::new())
}

#[derive(Debug, Clone)]
struct R6Compilation {
    files: Vec<R6CompiledFile>,
}

#[derive(Debug, Clone)]
struct R6CompiledFile {
    relative_path: PathBuf,
    contents: String,
}

#[derive(Debug, Clone, Deserialize)]
struct R6ImportFile {
    holons: Vec<R6ImportHolon>,
}

#[derive(Debug, Clone, Deserialize)]
struct R6ImportHolon {
    key: String,
    #[serde(rename = "type")]
    descriptor_type: String,
    #[allow(dead_code)]
    properties: serde_json::Map<String, Value>,
    #[serde(default)]
    relationships: Vec<R6ImportRelationship>,
}

#[derive(Debug, Clone, Deserialize)]
struct R6ImportRelationship {
    name: String,
    target: Vec<R6ImportReference>,
}

#[derive(Debug, Clone, Deserialize)]
struct R6ImportReference {
    #[serde(rename = "$ref")]
    ref_key: String,
}

fn build_r6_compilation(parsed_files: Vec<ParsedTdlFile>) -> Result<R6Compilation> {
    let mut seen_keys = HashMap::<String, PathBuf>::new();
    let schema_owner_paths = schema_owner_paths(&parsed_files);
    let mut files = Vec::with_capacity(parsed_files.len());

    for parsed in parsed_files {
        let emits_schema_holon = schema_owner_paths
            .get(&parsed.schema.name)
            .map(|owner_path| owner_path == &parsed.relative_path)
            .unwrap_or(true);
        let import_json =
            lower_r6_file_to_import_json(&parsed, emits_schema_holon, &mut seen_keys)?;
        let contents = serde_json::to_string_pretty(&import_json)?;
        validate_r6_import_json(&contents).with_context(|| {
            format!("validating generated JSON for {}", parsed.relative_path.display())
        })?;
        files.push(R6CompiledFile { relative_path: parsed.relative_path, contents });
    }

    Ok(R6Compilation { files })
}

fn schema_owner_paths(parsed_files: &[ParsedTdlFile]) -> HashMap<String, PathBuf> {
    let mut owner_paths = HashMap::<String, PathBuf>::new();
    for file in parsed_files {
        let schema_name = file.schema.name.clone();
        let existing_owner = owner_paths.get(&schema_name);
        let should_replace = existing_owner
            .map(|path| is_preferred_schema_owner(&file.relative_path, path))
            .unwrap_or(true);
        if should_replace {
            owner_paths.insert(schema_name, file.relative_path.clone());
        }
    }
    owner_paths
}

fn is_preferred_schema_owner(candidate: &Path, current: &Path) -> bool {
    let candidate_name = candidate.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
    let current_name = current.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();

    candidate_name.contains("root") && !current_name.contains("root")
}

fn validate_r6_import_json(raw_json: &str) -> Result<()> {
    let parsed: R6ImportFile = serde_json::from_str(raw_json)?;
    for holon in parsed.holons {
        if holon.key.trim().is_empty() {
            return Err(anyhow!("loader holon key cannot be empty"));
        }
        if holon.descriptor_type.trim().is_empty() {
            return Err(anyhow!("loader holon '{}' type cannot be empty", holon.key));
        }
        for relationship in holon.relationships {
            if relationship.name.trim().is_empty() {
                return Err(anyhow!("loader holon '{}' has empty relationship name", holon.key));
            }
            if relationship.target.is_empty() {
                return Err(anyhow!(
                    "loader holon '{}' relationship '{}' has no targets",
                    holon.key,
                    relationship.name
                ));
            }
            for target in relationship.target {
                if target.ref_key.trim().is_empty() {
                    return Err(anyhow!(
                        "loader holon '{}' relationship '{}' has empty target",
                        holon.key,
                        relationship.name
                    ));
                }
            }
        }
    }
    Ok(())
}

fn lower_r6_file_to_import_json(
    file: &ParsedTdlFile,
    emits_schema_holon: bool,
    seen_keys: &mut HashMap<String, PathBuf>,
) -> Result<Value> {
    let mut holons = Vec::new();
    if emits_schema_holon {
        let schema_holon = lower_r6_schema_holon(&file.schema)?;
        record_r6_key(&schema_holon.key, &file.relative_path, seen_keys)?;
        holons.push(schema_holon.into_json());
    }

    for descriptor in &file.descriptors {
        let holon = lower_r6_descriptor_holon(descriptor, &file.schema.name)?;
        record_r6_key(&holon.key, &file.relative_path, seen_keys)?;
        holons.push(holon.into_json());
    }

    let mut root = serde_json::Map::new();
    if !file.meta.is_empty() {
        root.insert(
            "meta".to_string(),
            literal_to_json(&TdlLiteralValue::Object(file.meta.clone())),
        );
    }
    root.insert("holons".to_string(), Value::Array(holons));
    Ok(Value::Object(root))
}

fn record_r6_key(key: &str, path: &Path, seen_keys: &mut HashMap<String, PathBuf>) -> Result<()> {
    if let Some(existing) = seen_keys.insert(key.to_string(), path.to_path_buf()) {
        return Err(anyhow!(
            "duplicate authored holon key `{key}` in {} and {}",
            existing.display(),
            path.display()
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct R6Holon {
    key: String,
    descriptor_type: String,
    properties: BTreeMap<String, Value>,
    relationships: BTreeMap<String, Vec<String>>,
}

impl R6Holon {
    fn new(key: String, descriptor_type: String) -> Self {
        Self { key, descriptor_type, properties: BTreeMap::new(), relationships: BTreeMap::new() }
    }

    fn property(&mut self, name: impl Into<String>, value: Value) -> Result<()> {
        let name = name.into();
        if self.properties.insert(name.clone(), value).is_some() {
            return Err(anyhow!("duplicate property `{name}` on `{}`", self.key));
        }
        Ok(())
    }

    fn relationship(&mut self, name: impl Into<String>, target: impl Into<String>) {
        self.relationships.entry(name.into()).or_default().push(target.into());
    }

    fn relationship_targets(
        &mut self,
        name: impl Into<String>,
        targets: impl IntoIterator<Item = String>,
    ) {
        self.relationships.entry(name.into()).or_default().extend(targets);
    }

    fn into_json(self) -> Value {
        let mut object = serde_json::Map::new();
        object.insert("key".to_string(), json!(self.key));
        object.insert("type".to_string(), json!(self.descriptor_type));
        object.insert("properties".to_string(), Value::Object(ordered_properties(self.properties)));
        if !self.relationships.is_empty() {
            object.insert(
                "relationships".to_string(),
                Value::Array(ordered_relationships(self.relationships)),
            );
        }
        Value::Object(object)
    }
}

fn lower_r6_schema_holon(schema: &TdlSchema) -> Result<R6Holon> {
    let mut holon = R6Holon::new(schema.name.clone(), "Schema.HolonType".to_string());
    holon.property("SchemaName", json!(schema.name.clone()))?;
    apply_r6_header(&mut holon, schema.header.as_ref())?;
    for (name, value) in schema.literal_properties.iter() {
        holon.property(name.clone(), literal_to_json(value))?;
    }
    if schema.allows_additional_properties {
        holon.property("AllowsAdditionalProperties", json!(true))?;
    }
    if schema.allows_additional_relationships {
        holon.property("AllowsAdditionalRelationships", json!(true))?;
    }
    for dependency in &schema.dependencies {
        holon.relationship("DependsOn", dependency.clone());
    }
    for relationship in &schema.literal_relationships {
        holon.relationship_targets(relationship.name.clone(), relationship.targets.clone());
    }
    Ok(holon)
}

fn lower_r6_descriptor_holon(descriptor: &TdlDescriptor, schema_name: &str) -> Result<R6Holon> {
    let key = descriptor_key_r6(descriptor)?;
    let descriptor_type = descriptor.descriptor_type.clone().ok_or_else(|| {
        anyhow!("declaration `{}` is missing required type clause", descriptor.name)
    })?;
    let mut holon = R6Holon::new(key.clone(), descriptor_type);

    if !descriptor.is_generic_instance {
        holon.property("TypeName", json!(local_type_name(descriptor, &key)))?;
    }
    if descriptor.is_abstract {
        holon.property("IsAbstractType", json!(true))?;
    }
    if descriptor.is_definitional {
        holon.property("IsDefinitional", json!(true))?;
    }
    if descriptor.is_ordered {
        holon.property("IsOrdered", json!(true))?;
    }
    if descriptor.allows_duplicates {
        holon.property("AllowsDuplicates", json!(true))?;
    }
    if descriptor.allows_additional_properties {
        holon.property("AllowsAdditionalProperties", json!(true))?;
    }
    if descriptor.allows_additional_relationships {
        holon.property("AllowsAdditionalRelationships", json!(true))?;
    }
    if let Some(min) = descriptor.min_cardinality {
        holon.property("MinCardinality", json!(min))?;
    }
    if let Some(max) = descriptor.max_cardinality {
        holon.property("MaxCardinality", json!(max))?;
    }
    if let Some(deletion_semantic) = &descriptor.deletion_semantic {
        holon.property("DeletionSemantic", json!(deletion_semantic))?;
    }
    apply_r6_header(&mut holon, descriptor.header.as_ref())?;
    for (name, value) in descriptor.literal_properties.iter() {
        holon.property(canonical_property_name(name), literal_to_json(value))?;
    }

    if descriptor.is_generic_instance
        && (descriptor.extends.is_some()
            || descriptor.value_type.is_some()
            || descriptor.source_type.is_some()
            || descriptor.target_type.is_some()
            || descriptor.key_rule.is_some()
            || descriptor.min_cardinality.is_some()
            || descriptor.max_cardinality.is_some()
            || descriptor.deletion_semantic.is_some()
            || descriptor.is_abstract
            || descriptor.is_definitional
            || descriptor.is_ordered
            || descriptor.allows_duplicates)
    {
        return Err(anyhow!(
            "generic instance `{}` must not use descriptor-only shorthand",
            descriptor.name
        ));
    }

    if let Some(extends) = &descriptor.extends {
        holon.relationship("Extends", extends.clone());
    }
    if !descriptor.is_generic_instance {
        holon.relationship("ComponentOf", schema_name.to_string());
    }
    if let Some(value_type) = &descriptor.value_type {
        holon.relationship("ValueType", value_type.clone());
    }
    if let Some(source_type) = &descriptor.source_type {
        holon.relationship("SourceType", source_type.clone());
    }
    if let Some(target_type) = &descriptor.target_type {
        holon.relationship("TargetType", target_type.clone());
    }
    if let Some(key_rule) = &descriptor.key_rule {
        holon.relationship("InstanceKeyRule", key_rule.clone());
    }
    if let Some(has_inverse) = &descriptor.has_inverse {
        holon.relationship("HasInverse", has_inverse.clone());
    }
    if descriptor.inverse_of.is_some() {
        return Err(anyhow!(
            "inverse relationship `{}` must not author inverse-side pair metadata",
            descriptor.name
        ));
    }
    for relationship in &descriptor.literal_relationships {
        if !descriptor.is_generic_instance && relationship.name == "ComponentOf" {
            return Err(anyhow!(
                "descriptor `{}` must not explicitly author ComponentOf in a schema file",
                descriptor.name
            ));
        }
        if relationship.name == "DescribedBy" {
            return Err(anyhow!(
                "descriptor `{}` must not author both type and DescribedBy",
                descriptor.name
            ));
        }
        if relationship.name == "Extends" && descriptor.extends.is_some() {
            return Err(anyhow!(
                "descriptor `{}` must not author both extends and Extends",
                descriptor.name
            ));
        }
        if relationship.name == "HasInverse" && descriptor.has_inverse.is_some() {
            // `HasInverse` is promoted to the relationship-pair semantic slot
            // and normalized to the inverse descriptor's full holon key. Do
            // not also emit its literal shorthand target: that would create a
            // second, unresolved loader reference such as `AuthorOf`.
            continue;
        }
        holon.relationship_targets(relationship.name.clone(), relationship.targets.clone());
    }
    if descriptor.kind == DescriptorKind::Enum && !descriptor.variants.is_empty() {
        holon.relationship_targets("Variants", descriptor.variants.clone());
    }

    assert_relationship_key_consistency(descriptor)?;
    Ok(holon)
}

fn apply_r6_header(holon: &mut R6Holon, header: Option<&DescriptorHeader>) -> Result<()> {
    let Some(header) = header else {
        return Ok(());
    };
    if let Some(description) = &header.description {
        holon.property("Description", json!(description))?;
    }
    if let Some(display_name) = &header.display_name {
        holon.property("DisplayName", json!(display_name))?;
    }
    if let Some(display_name_plural) = &header.display_name_plural {
        holon.property("DisplayNamePlural", json!(display_name_plural))?;
    }
    if let Some(type_name_plural) = &header.type_name_plural {
        holon.property("TypeNamePlural", json!(type_name_plural))?;
    }
    Ok(())
}

fn descriptor_key_r6(descriptor: &TdlDescriptor) -> Result<String> {
    match descriptor.kind {
        DescriptorKind::EnumVariant => Ok(descriptor
            .variant_of
            .as_ref()
            .map(|parent| variant_key(parent, &descriptor.name))
            .unwrap_or_else(|| descriptor.name.clone())),
        DescriptorKind::RelationshipType => {
            if descriptor.name.starts_with('(') {
                Ok(descriptor.name.clone())
            } else {
                let source = descriptor.source_type.clone().ok_or_else(|| {
                    anyhow!("relationship `{}` is missing source", descriptor.name)
                })?;
                let target = descriptor.target_type.clone().ok_or_else(|| {
                    anyhow!("relationship `{}` is missing target", descriptor.name)
                })?;
                Ok(format!("({source})-[{}]->({target})", descriptor.name))
            }
        }
        _ => Ok(descriptor.name.clone()),
    }
}

fn local_type_name(descriptor: &TdlDescriptor, key: &str) -> String {
    if descriptor.kind == DescriptorKind::RelationshipType {
        return relationship_name_from_key(key).unwrap_or(key).to_string();
    }
    if descriptor.kind == DescriptorKind::EnumVariant {
        return key.rsplit_once('.').map(|(_, local)| local).unwrap_or(key).to_string();
    }
    key.split('.').next().unwrap_or(key).to_string()
}

fn relationship_name_from_key(key: &str) -> Option<&str> {
    let (_, rest) = key.split_once(")-[")?;
    let (name, _) = rest.split_once("]->(")?;
    Some(name)
}

fn relationship_source_target_from_key(key: &str) -> Option<(&str, &str)> {
    let source = key.strip_prefix('(')?.split_once(")-[")?.0;
    let (_, target_with_close) = key.split_once("]->(")?;
    let target = target_with_close.strip_suffix(')')?;
    Some((source, target))
}

fn assert_relationship_key_consistency(descriptor: &TdlDescriptor) -> Result<()> {
    if descriptor.kind != DescriptorKind::RelationshipType || !descriptor.name.starts_with('(') {
        return Ok(());
    }
    let Some((key_source, key_target)) = relationship_source_target_from_key(&descriptor.name)
    else {
        return Err(anyhow!("invalid relationship key `{}`", descriptor.name));
    };
    if descriptor.source_type.as_deref() != Some(key_source) {
        return Err(anyhow!(
            "relationship `{}` source clause does not match key source `{key_source}`",
            descriptor.name
        ));
    }
    if descriptor.target_type.as_deref() != Some(key_target) {
        return Err(anyhow!(
            "relationship `{}` target clause does not match key target `{key_target}`",
            descriptor.name
        ));
    }
    Ok(())
}

fn literal_to_json(value: &TdlLiteralValue) -> Value {
    match value {
        TdlLiteralValue::Null => Value::Null,
        TdlLiteralValue::Boolean(value) => json!(value),
        TdlLiteralValue::Integer(value) => json!(value),
        TdlLiteralValue::Number(value) => {
            serde_json::from_str(value).unwrap_or_else(|_| json!(value))
        }
        TdlLiteralValue::String(value) => json!(value),
        TdlLiteralValue::Array(values) => {
            Value::Array(values.iter().map(literal_to_json).collect())
        }
        TdlLiteralValue::Object(object) => {
            let mut map = serde_json::Map::new();
            for (name, value) in object.iter() {
                map.insert(name.clone(), literal_to_json(value));
            }
            Value::Object(map)
        }
    }
}

fn json_value_to_tdl_literal(value: &Value) -> TdlLiteralValue {
    match value {
        Value::Null => TdlLiteralValue::Null,
        Value::Bool(value) => TdlLiteralValue::Boolean(*value),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                TdlLiteralValue::Integer(integer)
            } else {
                TdlLiteralValue::Number(number.to_string())
            }
        }
        Value::String(value) => TdlLiteralValue::String(value.clone()),
        Value::Array(values) => {
            TdlLiteralValue::Array(values.iter().map(json_value_to_tdl_literal).collect())
        }
        Value::Object(object) => TdlLiteralValue::Object(
            object
                .iter()
                .map(|(name, value)| (name.clone(), json_value_to_tdl_literal(value)))
                .collect(),
        ),
    }
}

fn canonical_property_name(name: &str) -> String {
    match name {
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
        "min_cardinality" => "MinCardinality",
        "max_cardinality" => "MaxCardinality",
        "deletion_semantic" => "DeletionSemantic",
        other => other,
    }
    .to_string()
}

fn ordered_properties(properties: BTreeMap<String, Value>) -> serde_json::Map<String, Value> {
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
        "MinCardinality",
        "MaxCardinality",
        "DeletionSemantic",
        "IsValueRequired",
        "DefaultValue",
        "AllowsAdditionalProperties",
        "AllowsAdditionalRelationships",
    ];
    order_map(properties, &preferred)
}

fn ordered_relationships(relationships: BTreeMap<String, Vec<String>>) -> Vec<Value> {
    let preferred = [
        "Extends",
        "ComponentOf",
        "DependsOn",
        "SourceType",
        "TargetType",
        "ValueType",
        "InstanceKeyRule",
        "HasInverse",
        "Variants",
        "InstanceProperties",
        "InstanceRelationships",
        "AffordsCommand",
        "AffordsDance",
        "AffordsOperator",
    ];
    let ordered = order_entries(relationships, &preferred);
    ordered
        .into_iter()
        .map(|(name, targets)| {
            json!({
                "name": name,
                "target": targets.into_iter().map(|target| json!({ "$ref": target })).collect::<Vec<_>>()
            })
        })
        .collect()
}

fn order_map(
    mut values: BTreeMap<String, Value>,
    preferred: &[&str],
) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    for key in preferred {
        if let Some(value) = values.remove(*key) {
            out.insert((*key).to_string(), value);
        }
    }
    for (key, value) in values {
        out.insert(key, value);
    }
    out
}

fn order_entries<T>(mut values: BTreeMap<String, T>, preferred: &[&str]) -> Vec<(String, T)> {
    let mut out = Vec::new();
    for key in preferred {
        if let Some(value) = values.remove(*key) {
            out.push(((*key).to_string(), value));
        }
    }
    out.extend(values);
    out
}

fn parse_inputs(inputs: &[PathBuf]) -> Result<Vec<ParsedTdlFile>> {
    let files = collect_tdl_files(inputs)?;
    let mut parsed = Vec::with_capacity(files.len());
    for discovered in files {
        let raw = fs::read_to_string(&discovered.source_path).with_context(|| {
            format!("reading TDL source file {}", discovered.source_path.display())
        })?;
        let document = parse_tdl_file(&raw, &discovered.relative_path)?;
        parsed.push(document);
    }
    Ok(parsed)
}

fn collect_tdl_files(inputs: &[PathBuf]) -> Result<Vec<DiscoveredFile>> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_dir() {
            collect_tdl_files_recursive(input, input, &mut files)?;
        } else if input.extension().and_then(|ext| ext.to_str()) == Some("tdl") {
            let relative_path =
                input.file_name().map(PathBuf::from).unwrap_or_else(|| input.clone());
            files.push(DiscoveredFile { source_path: input.clone(), relative_path });
        }
    }
    ensure_unique_relative_paths(&files)?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn collect_tdl_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut Vec<DiscoveredFile>,
) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("reading input directory {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_tdl_files_recursive(root, &path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("tdl") {
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

fn ensure_unique_relative_paths(files: &[DiscoveredFile]) -> Result<()> {
    let mut seen = std::collections::HashMap::<String, PathBuf>::new();
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

fn parse_tdl_file(raw: &str, relative_path: &Path) -> Result<ParsedTdlFile> {
    let mut parser = Parser::new(raw, relative_path);
    parser.parse_file()
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    index: usize,
    relative_path: PathBuf,
    pending_descriptors: Vec<TdlDescriptor>,
}

impl<'a> Parser<'a> {
    fn new(raw: &'a str, relative_path: &Path) -> Self {
        Self {
            lines: raw.lines().collect(),
            index: 0,
            relative_path: relative_path.to_path_buf(),
            pending_descriptors: Vec::new(),
        }
    }

    fn parse_file(&mut self) -> Result<ParsedTdlFile> {
        let file_path = self.relative_path.clone();
        let mut schema: Option<TdlSchema> = None;
        let mut meta = TdlLiteralObject::new();
        let mut descriptors = Vec::new();

        while self.skip_blank_lines() {
            let line = self.peek_trimmed().unwrap().to_string();
            if line == "meta {" {
                self.consume_trimmed();
                let (properties, references) = self.parse_properties_block()?;
                if !meta.is_empty() || !references.is_empty() {
                    return Err(anyhow!("invalid meta declaration in {}", file_path.display()));
                }
                meta = properties;
            } else if line.starts_with("schema ") {
                if schema.is_some() {
                    return Err(anyhow!("multiple schema declarations in {}", file_path.display()));
                }
                schema = Some(self.parse_schema_decl()?);
            } else if is_descriptor_line(&line) {
                descriptors.push(self.parse_descriptor_decl(None)?);
                descriptors.append(&mut self.pending_descriptors);
            } else if line == "}" {
                return Err(anyhow!("unexpected closing brace in {}", file_path.display()));
            } else {
                return Err(anyhow!(
                    "unrecognized top-level declaration in {}: {}",
                    file_path.display(),
                    line
                ));
            }
        }

        let schema = schema
            .ok_or_else(|| anyhow!("missing schema declaration in {}", file_path.display()))?;
        Ok(ParsedTdlFile { relative_path: file_path, meta, schema, descriptors })
    }

    fn parse_schema_decl(&mut self) -> Result<TdlSchema> {
        let line = self.consume_trimmed().unwrap();
        let header = parse_inline_header(&line, "schema")?;
        let name = header.name;
        let mut dependencies = Vec::new();
        let mut literal_properties = TdlLiteralObject::new();
        let mut literal_relationships = Vec::new();
        let mut allows_additional_properties = false;
        let mut allows_additional_relationships = false;
        let mut block_header: Option<DescriptorHeader> = None;

        if header.has_block || self.try_consume_open_brace()? {
            while self.skip_blank_lines() {
                let current = self.peek_trimmed().unwrap().to_string();
                if current == "}" {
                    self.consume_trimmed();
                    break;
                }
                if current.starts_with("depends_on ") {
                    dependencies
                        .push(parse_reference_token(current["depends_on ".len()..].trim())?);
                    self.consume_trimmed();
                } else if current == "properties {" {
                    self.consume_trimmed();
                    let (properties, _instance_properties) = self.parse_properties_block()?;
                    literal_properties
                        .extend(properties.iter().map(|(key, value)| (key.clone(), value.clone())));
                } else if current == "relationships {" {
                    self.consume_trimmed();
                    for line in self.parse_reference_block()? {
                        if let Some(relationship) = parse_literal_relationship_line(&line)? {
                            literal_relationships.push(relationship);
                        } else {
                            return Err(anyhow!("unexpected schema relationship line: {}", line));
                        }
                    }
                } else if current == "allows_additional_properties" {
                    allows_additional_properties = true;
                    self.consume_trimmed();
                } else if current == "allows_additional_relationships" {
                    allows_additional_relationships = true;
                    self.consume_trimmed();
                } else if current.starts_with("header") {
                    block_header = Some(self.parse_header_block()?);
                } else {
                    return Err(anyhow!("unexpected schema clause: {}", current));
                }
            }
        }

        Ok(TdlSchema {
            name,
            dependencies,
            literal_properties,
            literal_relationships,
            header: block_header.or(header.header),
            allows_additional_properties,
            allows_additional_relationships,
        })
    }

    fn parse_descriptor_decl(&mut self, variant_of: Option<String>) -> Result<TdlDescriptor> {
        let line = self.consume_trimmed().unwrap();
        let parsed = parse_descriptor_header(&line)?;
        let declaration_name = parsed.name.clone();
        let mut clauses = DescriptorClauseTracker::default();
        clauses.mark_if_present("extends", &declaration_name, parsed.extends.is_some())?;
        let mut descriptor = TdlDescriptor {
            kind: parsed.kind,
            name: parsed.name,
            header: None,
            is_generic_instance: parsed.is_generic_instance,
            is_abstract: parsed.is_abstract,
            relationship_flavor: parsed.relationship_flavor,
            descriptor_type: None,
            extends: parsed.extends,
            value_type: None,
            source_type: None,
            target_type: None,
            inverse_of: None,
            has_inverse: None,
            key_rule: None,
            min_cardinality: None,
            max_cardinality: None,
            deletion_semantic: None,
            is_ordered: false,
            allows_duplicates: false,
            allows_additional_properties: false,
            allows_additional_relationships: false,
            is_definitional: parsed.is_definitional,
            variants: Vec::new(),
            variant_of,
            literal_properties: TdlLiteralObject::new(),
            instance_properties: Vec::new(),
            instance_relationships: Vec::new(),
            literal_relationships: Vec::new(),
        };

        if parsed.has_block || self.try_consume_open_brace()? {
            while self.skip_blank_lines() {
                let current = self.peek_trimmed().unwrap().to_string();
                if current == "}" {
                    self.consume_trimmed();
                    break;
                }
                match current.as_str() {
                    s if s.starts_with("header") => {
                        clauses.mark("header", &declaration_name)?;
                        descriptor.header = Some(self.parse_header_block()?);
                    }
                    s if s.starts_with("extends ") => {
                        clauses.mark("extends", &declaration_name)?;
                        descriptor.extends =
                            Some(parse_reference_token(s["extends ".len()..].trim())?);
                        self.consume_trimmed();
                    }
                    s if s.starts_with("type ") => {
                        clauses.mark("type", &declaration_name)?;
                        descriptor.descriptor_type =
                            Some(parse_reference_token(s["type ".len()..].trim())?);
                        self.consume_trimmed();
                    }
                    s if s.starts_with("value ") => {
                        clauses.mark("value", &declaration_name)?;
                        descriptor.value_type =
                            Some(parse_reference_token(s["value ".len()..].trim())?);
                        self.consume_trimmed();
                    }
                    s if s.starts_with("source ") => {
                        clauses.mark("source", &declaration_name)?;
                        descriptor.source_type =
                            Some(parse_reference_token(s["source ".len()..].trim())?);
                        self.consume_trimmed();
                    }
                    s if s.starts_with("target ") => {
                        clauses.mark("target", &declaration_name)?;
                        descriptor.target_type =
                            Some(parse_reference_token(s["target ".len()..].trim())?);
                        self.consume_trimmed();
                    }
                    s if s.starts_with("inverse ") => {
                        clauses.mark("inverse", &declaration_name)?;
                        let inverse_name = s["inverse ".len()..].trim().to_string();
                        if descriptor.relationship_flavor == Some(RelationshipFlavor::Inverse) {
                            descriptor.inverse_of = Some(inverse_name);
                        } else {
                            descriptor.has_inverse = Some(inverse_name);
                        }
                        self.consume_trimmed();
                    }
                    s if s.starts_with("instance_keyrule ") => {
                        clauses.mark("keyrule", &declaration_name)?;
                        descriptor.key_rule =
                            Some(parse_reference_token(s["instance_keyrule ".len()..].trim())?);
                        self.consume_trimmed();
                    }
                    s if s.starts_with("keyrule ") => {
                        clauses.mark("keyrule", &declaration_name)?;
                        descriptor.key_rule =
                            Some(parse_reference_token(s["keyrule ".len()..].trim())?);
                        self.consume_trimmed();
                    }
                    s if s.starts_with("cardinality ") => {
                        clauses.mark("cardinality", &declaration_name)?;
                        let range = s["cardinality ".len()..].trim();
                        let (min, max) = range
                            .split_once("..")
                            .ok_or_else(|| anyhow!("invalid cardinality '{}'", range))?;
                        descriptor.min_cardinality = Some(min.trim().parse()?);
                        descriptor.max_cardinality =
                            if max.trim() == "*" { None } else { Some(max.trim().parse()?) };
                        self.consume_trimmed();
                    }
                    "ordered" => {
                        clauses.mark("ordered", &declaration_name)?;
                        descriptor.is_ordered = true;
                        self.consume_trimmed();
                    }
                    "duplicates" => {
                        clauses.mark("duplicates", &declaration_name)?;
                        descriptor.allows_duplicates = true;
                        self.consume_trimmed();
                    }
                    "allows_additional_properties" => {
                        clauses.mark("allows_additional_properties", &declaration_name)?;
                        descriptor.allows_additional_properties = true;
                        self.consume_trimmed();
                    }
                    "allows_additional_relationships" => {
                        clauses.mark("allows_additional_relationships", &declaration_name)?;
                        descriptor.allows_additional_relationships = true;
                        self.consume_trimmed();
                    }
                    s if s.starts_with("deletion_semantic ") => {
                        clauses.mark("deletion_semantic", &declaration_name)?;
                        descriptor.deletion_semantic =
                            Some(s["deletion_semantic ".len()..].trim().to_string());
                        self.consume_trimmed();
                    }
                    "properties {" => {
                        self.consume_trimmed();
                        let (literal_properties, instance_properties) =
                            self.parse_properties_block()?;
                        descriptor.literal_properties.extend(
                            literal_properties
                                .iter()
                                .map(|(key, value)| (key.clone(), value.clone())),
                        );
                        descriptor.instance_properties.extend(instance_properties);
                    }
                    "relationships {" => {
                        self.consume_trimmed();
                        descriptor.literal_relationships.extend(self.parse_relationship_map()?);
                    }
                    s if s.starts_with("relationships {") => {
                        return Err(anyhow!(
                            "relationship maps must use a newline-oriented braced block: {}",
                            s
                        ));
                    }
                    "variants {" if descriptor.kind == DescriptorKind::Enum => {
                        self.consume_trimmed();
                        for variant in self.parse_variant_block(&descriptor.name)? {
                            let variant_key = variant_key(&descriptor.name, &variant.name);
                            self.pending_descriptors.push(variant.clone());
                            descriptor.variants.push(variant_key);
                        }
                    }
                    other => {
                        if descriptor.kind == DescriptorKind::Enum && other.starts_with("variant ")
                        {
                            // Allow inline variant declarations if the parser encounters them
                            // outside a nested variants block.
                            let variant = self.parse_variant_decl(Some(descriptor.name.clone()))?;
                            let variant_key = variant_key(&descriptor.name, &variant.name);
                            self.pending_descriptors.push(variant);
                            descriptor.variants.push(variant_key);
                        } else {
                            let Some((name, value)) = parse_fixed_property_clause(other)? else {
                                return Err(anyhow!("unexpected descriptor clause: {}", other));
                            };
                            descriptor.literal_properties.insert(name, value);
                            self.consume_trimmed();
                        }
                    }
                }
            }
        }

        apply_literal_properties_to_tdl_descriptor(&mut descriptor)?;
        apply_literal_relationships_to_tdl_descriptor(&mut descriptor);
        normalize_relationship_pair_targets(&mut descriptor);
        Ok(descriptor)
    }

    fn parse_variant_decl(&mut self, variant_of: Option<String>) -> Result<TdlDescriptor> {
        let line = self.consume_trimmed().unwrap();
        let parsed = parse_descriptor_header(&line)?;
        if parsed.kind != DescriptorKind::EnumVariant {
            return Err(anyhow!("expected variant declaration, found {}", line));
        }
        let declaration_name = parsed.name.clone();
        let mut clauses = DescriptorClauseTracker::default();
        clauses.mark_if_present("extends", &declaration_name, parsed.extends.is_some())?;
        let mut descriptor = TdlDescriptor {
            kind: DescriptorKind::EnumVariant,
            name: parsed.name,
            header: None,
            is_generic_instance: parsed.is_generic_instance,
            is_abstract: parsed.is_abstract,
            relationship_flavor: parsed.relationship_flavor,
            descriptor_type: None,
            extends: parsed.extends,
            value_type: None,
            source_type: None,
            target_type: None,
            inverse_of: None,
            has_inverse: None,
            key_rule: None,
            min_cardinality: None,
            max_cardinality: None,
            deletion_semantic: None,
            is_ordered: false,
            allows_duplicates: false,
            allows_additional_properties: false,
            allows_additional_relationships: false,
            is_definitional: false,
            variants: Vec::new(),
            variant_of,
            literal_properties: TdlLiteralObject::new(),
            instance_properties: Vec::new(),
            instance_relationships: Vec::new(),
            literal_relationships: Vec::new(),
        };

        if parsed.has_block || self.try_consume_open_brace()? {
            while self.skip_blank_lines() {
                let current = self.peek_trimmed().unwrap().to_string();
                if current == "}" {
                    self.consume_trimmed();
                    break;
                }
                if current.starts_with("header") {
                    clauses.mark("header", &declaration_name)?;
                    descriptor.header = Some(self.parse_header_block()?);
                } else if current == "properties {" {
                    self.consume_trimmed();
                    let (literal_properties, instance_properties) =
                        self.parse_properties_block()?;
                    descriptor.literal_properties.extend(
                        literal_properties.iter().map(|(key, value)| (key.clone(), value.clone())),
                    );
                    descriptor.instance_properties.extend(instance_properties);
                } else if current == "relationships {" {
                    self.consume_trimmed();
                    descriptor.literal_relationships.extend(self.parse_relationship_map()?);
                } else if current.starts_with("type ") {
                    clauses.mark("type", &declaration_name)?;
                    descriptor.descriptor_type =
                        Some(parse_reference_token(current["type ".len()..].trim())?);
                    self.consume_trimmed();
                } else if current.starts_with("extends ") {
                    clauses.mark("extends", &declaration_name)?;
                    descriptor.extends =
                        Some(parse_reference_token(current["extends ".len()..].trim())?);
                    self.consume_trimmed();
                } else {
                    return Err(anyhow!("unexpected variant clause: {}", current));
                }
            }
        }

        apply_literal_properties_to_tdl_descriptor(&mut descriptor)?;
        apply_literal_relationships_to_tdl_descriptor(&mut descriptor);
        normalize_relationship_pair_targets(&mut descriptor);
        Ok(descriptor)
    }

    fn parse_variant_block(&mut self, enum_name: &str) -> Result<Vec<TdlDescriptor>> {
        let mut variants = Vec::new();
        while self.skip_blank_lines() {
            let current = self.peek_trimmed().unwrap().to_string();
            if current == "}" {
                self.consume_trimmed();
                break;
            }
            if current.starts_with("variant ") {
                variants.push(self.parse_variant_decl(Some(enum_name.to_string()))?);
            } else {
                return Err(anyhow!("unexpected variants clause: {}", current));
            }
        }
        Ok(variants)
    }

    fn parse_header_block(&mut self) -> Result<DescriptorHeader> {
        let line = self.consume_trimmed().unwrap();
        if !line.starts_with("header") {
            return Err(anyhow!("expected header block, found {}", line));
        }
        if !line.trim_end().ends_with('{') {
            self.expect_open_brace()?;
        }
        let mut description = None;
        let mut display_name = None;
        let mut display_name_plural = None;
        let mut type_name_plural = None;

        while self.skip_blank_lines() {
            let current = self.peek_trimmed().unwrap().to_string();
            if current == "}" {
                self.consume_trimmed();
                break;
            }
            let (field, value) = current
                .split_once(':')
                .ok_or_else(|| anyhow!("invalid header field '{}'", current))?;
            let value = parse_string_literal(value.trim())?;
            match field.trim() {
                "description" => description = Some(value),
                "display_name" => display_name = Some(value),
                "display_plural" => display_name_plural = Some(value),
                "plural" => type_name_plural = Some(value),
                other => return Err(anyhow!("unexpected header field '{}'", other)),
            }
            self.consume_trimmed();
        }

        Ok(DescriptorHeader { description, display_name, display_name_plural, type_name_plural })
    }

    fn parse_reference_block(&mut self) -> Result<Vec<String>> {
        let mut refs = Vec::new();
        while self.skip_blank_lines() {
            let current = self.peek_trimmed().unwrap().to_string();
            if current == "}" {
                self.consume_trimmed();
                break;
            }
            refs.push(current);
            self.consume_trimmed();
        }
        Ok(refs)
    }

    fn parse_relationship_map(&mut self) -> Result<Vec<LiteralRelationship>> {
        let mut relationships = Vec::new();
        while self.skip_blank_lines() {
            let current = self.peek_trimmed().unwrap().trim_end_matches(',').trim().to_string();
            if current == "}" {
                self.consume_trimmed();
                break;
            }

            let (name, raw_targets) = current
                .split_once("->")
                .ok_or_else(|| anyhow!("invalid relationship map entry '{}'", current))?;
            let name = name.trim().to_string();
            let raw_targets = raw_targets.trim();
            self.consume_trimmed();

            let targets = if raw_targets == "[" {
                self.parse_relationship_target_list()?
            } else if raw_targets.starts_with('[') {
                parse_inline_target_list(raw_targets)?
            } else {
                vec![parse_reference_token(raw_targets)?]
            };

            relationships.push(LiteralRelationship { name, targets });
        }
        Ok(relationships)
    }

    fn parse_relationship_target_list(&mut self) -> Result<Vec<String>> {
        let mut targets = Vec::new();
        while self.skip_blank_lines() {
            let current = self.peek_trimmed().unwrap().trim().to_string();
            if current == "]" || current == "]," {
                self.consume_trimmed();
                break;
            }
            targets.push(parse_reference_token(&current)?);
            self.consume_trimmed();
        }
        Ok(targets)
    }

    fn parse_properties_block(&mut self) -> Result<(TdlLiteralObject, Vec<String>)> {
        let mut properties = TdlLiteralObject::new();
        let mut refs = Vec::new();
        while self.skip_blank_lines() {
            let current = self.peek_trimmed().unwrap().to_string();
            if current == "}" {
                self.consume_trimmed();
                break;
            }
            if let Some((name, value)) = parse_literal_property_line(&current)? {
                properties.insert(name, value);
            } else {
                refs.push(current);
            }
            self.consume_trimmed();
        }
        Ok((properties, refs))
    }

    fn skip_blank_lines(&mut self) -> bool {
        while let Some(line) = self.peek_raw() {
            if line.trim().is_empty() {
                self.index += 1;
                continue;
            }
            if line.trim_start().starts_with("//") {
                self.index += 1;
                continue;
            }
            return true;
        }
        false
    }

    fn peek_raw(&self) -> Option<&'a str> {
        self.lines.get(self.index).copied()
    }

    fn peek_trimmed(&self) -> Option<&'a str> {
        self.peek_raw().map(str::trim)
    }

    fn consume_trimmed(&mut self) -> Option<&'a str> {
        let value = self.peek_raw();
        if value.is_some() {
            self.index += 1;
        }
        value.map(str::trim)
    }

    fn try_consume_open_brace(&mut self) -> Result<bool> {
        if self.skip_blank_lines() && self.peek_trimmed() == Some("{") {
            self.index += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn expect_open_brace(&mut self) -> Result<()> {
        if self.try_consume_open_brace()? {
            Ok(())
        } else {
            Err(anyhow!("expected '{{'"))
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedHead {
    kind: DescriptorKind,
    name: String,
    is_abstract: bool,
    is_definitional: bool,
    is_generic_instance: bool,
    relationship_flavor: Option<RelationshipFlavor>,
    extends: Option<String>,
    has_block: bool,
}

fn parse_inline_header(line: &str, keyword: &str) -> Result<InlineHeader> {
    let body = line.trim();
    if !body.starts_with(keyword) {
        return Err(anyhow!("expected {} declaration", keyword));
    }
    let mut remainder = body[keyword.len()..].trim();
    let has_block = remainder.ends_with('{');
    if has_block {
        remainder = remainder.trim_end_matches('{').trim();
    }
    if remainder.is_empty() {
        return Err(anyhow!("missing {} name", keyword));
    }
    Ok(InlineHeader { name: parse_reference_token(remainder)?, header: None, has_block })
}

struct InlineHeader {
    name: String,
    header: Option<DescriptorHeader>,
    has_block: bool,
}

#[derive(Debug, Default)]
struct DescriptorClauseTracker {
    header: bool,
    extends: bool,
    descriptor_type: bool,
    value_type: bool,
    source_type: bool,
    target_type: bool,
    inverse_pair: bool,
    key_rule: bool,
    cardinality: bool,
    ordered: bool,
    duplicates: bool,
    allows_additional_properties: bool,
    allows_additional_relationships: bool,
    deletion_semantic: bool,
}

impl DescriptorClauseTracker {
    fn mark(&mut self, clause: &'static str, declaration_name: &str) -> Result<()> {
        let slot = match clause {
            "header" => &mut self.header,
            "extends" => &mut self.extends,
            "type" => &mut self.descriptor_type,
            "value" => &mut self.value_type,
            "source" => &mut self.source_type,
            "target" => &mut self.target_type,
            "inverse" => &mut self.inverse_pair,
            "keyrule" => &mut self.key_rule,
            "cardinality" => &mut self.cardinality,
            "ordered" => &mut self.ordered,
            "duplicates" => &mut self.duplicates,
            "allows_additional_properties" => &mut self.allows_additional_properties,
            "allows_additional_relationships" => &mut self.allows_additional_relationships,
            "deletion_semantic" => &mut self.deletion_semantic,
            _ => unreachable!("untracked TDL singleton clause"),
        };
        if *slot {
            return Err(anyhow!("duplicate `{clause}` clause in declaration `{declaration_name}`"));
        }
        *slot = true;
        Ok(())
    }

    fn mark_if_present(
        &mut self,
        clause: &'static str,
        declaration_name: &str,
        present: bool,
    ) -> Result<()> {
        if present {
            self.mark(clause, declaration_name)?;
        }
        Ok(())
    }
}

fn parse_descriptor_header(line: &str) -> Result<ParsedHead> {
    let trimmed = line.trim();
    let has_block = trimmed.ends_with('{');
    let head = if has_block { trimmed.trim_end_matches('{').trim() } else { trimmed };
    let mut is_abstract = false;
    let mut is_definitional = false;
    let mut is_generic_instance = false;
    let mut extends = None;

    let mut remainder = head;
    if remainder.starts_with("abstract ") {
        is_abstract = true;
        remainder = remainder["abstract ".len()..].trim();
    }

    let (kind, after_kind) = if remainder.starts_with("def relationship ") {
        is_definitional = true;
        (DescriptorKind::RelationshipType, remainder["def relationship ".len()..].trim())
    } else if remainder.starts_with("inverse relationship ") {
        (DescriptorKind::RelationshipType, remainder["inverse relationship ".len()..].trim())
    } else {
        let (kind, tail) = if let Some(tail) = remainder.strip_prefix("holon ") {
            (DescriptorKind::HolonType, tail)
        } else if let Some(tail) = remainder.strip_prefix("value ") {
            (DescriptorKind::ValueType, tail)
        } else if let Some(tail) = remainder.strip_prefix("enum ") {
            (DescriptorKind::Enum, tail)
        } else if let Some(tail) = remainder.strip_prefix("property ") {
            (DescriptorKind::PropertyType, tail)
        } else if let Some(tail) = remainder.strip_prefix("relationship ") {
            (DescriptorKind::RelationshipType, tail)
        } else if let Some(tail) = remainder.strip_prefix("instance ") {
            is_generic_instance = true;
            (DescriptorKind::HolonType, tail)
        } else if let Some(tail) = remainder.strip_prefix("variant ") {
            (DescriptorKind::EnumVariant, tail)
        } else {
            return Err(anyhow!("unrecognized TDL declaration: {}", line));
        };
        (kind, tail.trim())
    };

    if after_kind.is_empty() {
        return Err(anyhow!("missing declaration name in '{}'", line));
    }
    let name = parse_reference_token(after_kind.trim_end_matches('{').trim())?;
    if kind == DescriptorKind::RelationshipType {
        if let Some((_, remainder)) = name.split_once(" extends ") {
            extends = Some(remainder.trim().to_string());
        }
    }

    let relationship_flavor = if kind == DescriptorKind::RelationshipType {
        Some(if head.starts_with("inverse relationship ") {
            RelationshipFlavor::Inverse
        } else {
            RelationshipFlavor::Declared
        })
    } else {
        None
    };

    Ok(ParsedHead {
        kind,
        name,
        is_abstract,
        is_definitional,
        is_generic_instance,
        relationship_flavor,
        extends,
        has_block,
    })
}

fn is_descriptor_line(line: &str) -> bool {
    line.starts_with("abstract ")
        || line.starts_with("def relationship ")
        || line.starts_with("inverse relationship ")
        || line.starts_with("holon ")
        || line.starts_with("value ")
        || line.starts_with("enum ")
        || line.starts_with("property ")
        || line.starts_with("relationship ")
        || line.starts_with("instance ")
        || line.starts_with("variant ")
}

fn parse_string_literal(raw: &str) -> Result<String> {
    if raw.starts_with('"') {
        Ok(serde_json::from_str(raw)?)
    } else {
        Ok(raw.to_string())
    }
}

fn parse_reference_token(raw: &str) -> Result<String> {
    let token = raw.trim().trim_end_matches(',').trim();
    if token.starts_with('"') {
        Ok(serde_json::from_str(token)?)
    } else {
        Ok(token.strip_prefix('#').unwrap_or(token).to_string())
    }
}

fn parse_inline_target_list(raw: &str) -> Result<Vec<String>> {
    let trimmed = raw.trim().trim_end_matches(',').trim();
    let Some(inner) = trimmed.strip_prefix('[').and_then(|value| value.strip_suffix(']')) else {
        return Err(anyhow!("invalid inline target list '{}'", raw));
    };
    inner
        .split(',')
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(parse_reference_token)
        .collect()
}

fn parse_fixed_property_clause(line: &str) -> Result<Option<(String, TdlLiteralValue)>> {
    let Some((name, raw_value)) = line.split_once(char::is_whitespace) else {
        return Ok(None);
    };
    let name = name.trim();
    let raw_value = raw_value.trim();
    if name.is_empty() || raw_value.is_empty() || name == "variant" {
        return Ok(None);
    }
    let value = if raw_value.starts_with('"') {
        serde_json::from_str::<Value>(raw_value)?
    } else if raw_value == "true" || raw_value == "false" {
        json!(raw_value == "true")
    } else if let Ok(integer) = raw_value.parse::<i64>() {
        json!(integer)
    } else {
        json!(parse_reference_token(raw_value)?)
    };
    Ok(Some((name.to_string(), json_value_to_tdl_literal(&value))))
}

fn parse_literal_relationship_line(line: &str) -> Result<Option<LiteralRelationship>> {
    if line.starts_with('(') {
        return Ok(None);
    }

    let Some((name, raw_targets)) = line.split_once("->") else {
        return Ok(None);
    };

    let name = name.trim();
    let raw_targets = raw_targets.trim();
    if name.is_empty() || raw_targets.is_empty() {
        return Ok(None);
    }

    let targets = if raw_targets.starts_with('[') {
        serde_json::from_str::<Vec<String>>(raw_targets)?
    } else if raw_targets.starts_with('"') {
        vec![serde_json::from_str::<String>(raw_targets)?]
    } else {
        vec![raw_targets.to_string()]
    };

    Ok(Some(LiteralRelationship { name: name.to_string(), targets }))
}

fn parse_literal_property_line(line: &str) -> Result<Option<(String, TdlLiteralValue)>> {
    let Some((name, raw_value)) = line.split_once(':') else {
        return Ok(None);
    };

    let name = name.trim();
    let raw_value = raw_value.trim();
    if name.is_empty() || raw_value.is_empty() {
        return Ok(None);
    }

    Ok(Some((name.to_string(), json_value_to_tdl_literal(&serde_json::from_str(raw_value)?))))
}

fn apply_literal_properties_to_tdl_descriptor(descriptor: &mut TdlDescriptor) -> Result<()> {
    if descriptor.literal_properties.is_empty() {
        return Ok(());
    }

    descriptor.is_abstract = descriptor
        .literal_properties
        .get("is_abstract_type")
        .and_then(|value| value.as_bool())
        .unwrap_or(descriptor.is_abstract);
    descriptor.allows_additional_properties = descriptor
        .literal_properties
        .get("allows_additional_properties")
        .and_then(|value| value.as_bool())
        .unwrap_or(descriptor.allows_additional_properties);
    descriptor.allows_additional_relationships = descriptor
        .literal_properties
        .get("allows_additional_relationships")
        .and_then(|value| value.as_bool())
        .unwrap_or(descriptor.allows_additional_relationships);
    descriptor.is_definitional = descriptor
        .literal_properties
        .get("is_definitional")
        .and_then(|value| value.as_bool())
        .unwrap_or(descriptor.is_definitional);
    descriptor.is_ordered = descriptor
        .literal_properties
        .get("is_ordered")
        .and_then(|value| value.as_bool())
        .unwrap_or(descriptor.is_ordered);
    descriptor.allows_duplicates = descriptor
        .literal_properties
        .get("allows_duplicates")
        .and_then(|value| value.as_bool())
        .unwrap_or(descriptor.allows_duplicates);
    descriptor.min_cardinality = descriptor
        .literal_properties
        .get("min_cardinality")
        .and_then(|value| value.as_i64())
        .or(descriptor.min_cardinality);
    descriptor.max_cardinality = descriptor
        .literal_properties
        .get("max_cardinality")
        .and_then(|value| value.as_i64())
        .or(descriptor.max_cardinality);
    descriptor.deletion_semantic = descriptor
        .literal_properties
        .get("deletion_semantic")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .or_else(|| descriptor.deletion_semantic.clone());

    let header = descriptor.header.get_or_insert(DescriptorHeader {
        description: None,
        display_name: None,
        display_name_plural: None,
        type_name_plural: None,
    });
    header.description = descriptor
        .literal_properties
        .get("description")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .or_else(|| header.description.clone());
    header.display_name = descriptor
        .literal_properties
        .get("display_name")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .or_else(|| header.display_name.clone());
    header.display_name_plural = descriptor
        .literal_properties
        .get("display_name_plural")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .or_else(|| header.display_name_plural.clone());
    header.type_name_plural = descriptor
        .literal_properties
        .get("type_name_plural")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .or_else(|| header.type_name_plural.clone());

    Ok(())
}

fn apply_literal_relationships_to_tdl_descriptor(descriptor: &mut TdlDescriptor) {
    if descriptor.is_generic_instance {
        return;
    }

    for relationship in &descriptor.literal_relationships {
        match relationship.name.as_str() {
            "Extends" if descriptor.extends.is_none() => {
                descriptor.extends = relationship.targets.first().cloned();
            }
            "InstanceKeyRule" if descriptor.key_rule.is_none() => {
                descriptor.key_rule = relationship.targets.first().cloned();
            }
            "SourceType" if descriptor.source_type.is_none() => {
                descriptor.source_type = relationship.targets.first().cloned();
            }
            "TargetType" if descriptor.target_type.is_none() => {
                descriptor.target_type = relationship.targets.first().cloned();
            }
            "InverseOf" if descriptor.inverse_of.is_none() => {
                descriptor.inverse_of = relationship.targets.first().cloned();
            }
            "HasInverse" if descriptor.has_inverse.is_none() => {
                descriptor.has_inverse = relationship.targets.first().cloned();
            }
            "Variants" => {
                for target in &relationship.targets {
                    if !descriptor.variants.contains(target) {
                        descriptor.variants.push(target.clone());
                    }
                }
            }
            "ValueType" if descriptor.value_type.is_none() => {
                descriptor.value_type = relationship.targets.first().cloned();
            }
            "VariantOf" if descriptor.variant_of.is_none() => {
                descriptor.variant_of = relationship.targets.first().cloned();
            }
            "InstanceProperties" => {
                for target in &relationship.targets {
                    if !descriptor.instance_properties.contains(target) {
                        descriptor.instance_properties.push(target.clone());
                    }
                }
            }
            "InstanceRelationships" => {
                for target in &relationship.targets {
                    if !descriptor.instance_relationships.contains(target) {
                        descriptor.instance_relationships.push(target.clone());
                    }
                }
            }
            _ => {}
        }
    }
}

fn normalize_relationship_pair_targets(descriptor: &mut TdlDescriptor) {
    let Some(source_type) = descriptor.source_type.clone() else {
        return;
    };
    let Some(target_type) = descriptor.target_type.clone() else {
        return;
    };

    if let Some(has_inverse) = descriptor.has_inverse.as_mut() {
        if !has_inverse.contains(")-[") {
            *has_inverse = format!("({target_type})-[{has_inverse}]->({source_type})");
        }
    }

    if let Some(inverse_of) = descriptor.inverse_of.as_mut() {
        if !inverse_of.contains(")-[") {
            *inverse_of = format!("({target_type})-[{inverse_of}]->({source_type})");
        }
    }
}

fn variant_key(enum_name: &str, variant_name: &str) -> String {
    format!("{enum_name}.{variant_name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile_inputs;
    use std::{
        env, fs,
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("schema-src")
    }

    fn temp_out_dir() -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("map-schema-compile-{nanos}"))
    }

    fn temp_tdl_dir() -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("map-schema-tdl-{nanos}"))
    }

    fn write_temp_tdl(file_name: &str, contents: &str) -> Result<PathBuf> {
        let dir = temp_tdl_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(file_name);
        fs::write(&path, contents)?;
        Ok(dir)
    }

    fn write_tdl_file(path: &Path, contents: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }

    fn discovered_tdl_file_count(root: &Path) -> Result<usize> {
        Ok(collect_tdl_files(&[root.to_path_buf()])?.len())
    }

    fn assert_check_rejects_duplicate_clause(clause: &str, declaration: &str, expected_name: &str) {
        let error = check_input_string(declaration, "duplicate-clause.tdl")
            .expect_err("duplicate singleton clause should be rejected");
        let message = error.to_string();
        assert!(
            message.contains(&format!("duplicate `{clause}` clause")),
            "expected duplicate `{clause}` error, got: {message}"
        );
        assert!(
            message.contains(&format!("declaration `{expected_name}`")),
            "expected declaration name `{expected_name}` in error, got: {message}"
        );
    }

    #[test]
    fn check_rejects_duplicate_descriptor_singleton_clauses() {
        let cases = [
            (
                "type",
                "Person.HolonType",
                r#"schema Example Schema-v0.0.1

holon Person.HolonType {
  type A.Type
  type B.Type
}
"#,
            ),
            (
                "extends",
                "Person.HolonType",
                r#"schema Example Schema-v0.0.1

holon Person.HolonType {
  type A.Type
  extends BaseA.HolonType
  extends BaseB.HolonType
}
"#,
            ),
            (
                "value",
                "Name.PropertyType",
                r#"schema Example Schema-v0.0.1

property Name.PropertyType {
  type MetaPropertyType.MetaTypeDescriptor
  value String.ValueType
  value Text.ValueType
}
"#,
            ),
            (
                "source",
                "Knows",
                r#"schema Example Schema-v0.0.1

relationship Knows {
  type MetaRelationshipType.MetaTypeDescriptor
  source Person.HolonType
  source Agent.HolonType
  target Person.HolonType
}
"#,
            ),
            (
                "target",
                "Knows",
                r#"schema Example Schema-v0.0.1

relationship Knows {
  type MetaRelationshipType.MetaTypeDescriptor
  source Person.HolonType
  target Person.HolonType
  target Agent.HolonType
}
"#,
            ),
            (
                "cardinality",
                "Knows",
                r#"schema Example Schema-v0.0.1

relationship Knows {
  type MetaRelationshipType.MetaTypeDescriptor
  source Person.HolonType
  target Person.HolonType
  cardinality 0..*
  cardinality 1..1
}
"#,
            ),
            (
                "inverse",
                "Knows",
                r#"schema Example Schema-v0.0.1

relationship Knows {
  type MetaRelationshipType.MetaTypeDescriptor
  source Person.HolonType
  target Person.HolonType
  inverse KnownBy
  inverse KnownAlsoBy
}
"#,
            ),
            (
                "keyrule",
                "Person.HolonType",
                r#"schema Example Schema-v0.0.1

holon Person.HolonType {
  type A.Type
  instance_keyrule RuleA.KeyRuleType
  keyrule RuleB.KeyRuleType
}
"#,
            ),
        ];

        for (clause, expected_name, declaration) in cases {
            assert_check_rejects_duplicate_clause(clause, declaration, expected_name);
        }
    }

    #[test]
    fn check_rejects_duplicate_variant_singleton_clauses() {
        assert_check_rejects_duplicate_clause(
            "type",
            r#"schema Example Schema-v0.0.1

enum Color.EnumType {
  type MetaEnumType.MetaTypeDescriptor
  variants {
    variant Red {
      type A.Type
      type B.Type
    }

  }
}
"#,
            "Red",
        );
    }

    #[test]
    fn check_rejects_inline_relationship_maps() {
        let error = check_input_string(
            r#"schema Example.Schema

holon Example.HolonType {
  type MetaHolonType.MetaTypeDescriptor
  relationships { InstanceProperties -> Example.PropertyType }
}
"#,
            "inline-relationship-map.tdl",
        )
        .expect_err("inline relationship maps are not part of the TDL grammar");

        assert!(error
            .to_string()
            .contains("relationship maps must use a newline-oriented braced block"));
    }

    #[test]
    fn core_schema_check_accepts_tdl_v09_corpus() -> Result<()> {
        let fixture_root = fixture_dir();
        assert_eq!(discovered_tdl_file_count(&fixture_root)?, 16);

        let diagnostics = check_inputs(&[fixture_root])?;

        assert!(diagnostics.is_empty());
        Ok(())
    }

    #[test]
    fn core_schema_check_output_reports_no_diagnostics() -> Result<()> {
        let diagnostics = check_inputs(&[fixture_dir()])?;

        assert_eq!(render_check_output(&diagnostics), "no diagnostics\n");
        Ok(())
    }

    #[test]
    fn core_schema_r6_compilation_builds_one_file_per_source_file() -> Result<()> {
        let fixture_root = fixture_dir();
        let parsed = parse_inputs(&[fixture_root.clone()])?;
        let compilation = build_r6_compilation(parsed)?;

        assert_eq!(discovered_tdl_file_count(&fixture_root)?, 16);
        assert_eq!(compilation.files.len(), 16);
        assert!(compilation
            .files
            .iter()
            .any(|file| { file.relative_path == PathBuf::from("core/root.tdl") }));

        Ok(())
    }

    #[test]
    fn core_schema_json_compilation_emits_canonical_schema_20_import_shape() -> Result<()> {
        let fixture_root = fixture_dir();
        let out_dir = temp_out_dir();
        let compiled_files = compile_inputs(&[fixture_root.clone()], &out_dir)?;

        assert_eq!(discovered_tdl_file_count(&fixture_root)?, 16);
        assert_eq!(compiled_files.len(), 16);

        let root_json = fs::read_to_string(out_dir.join("core/root.json"))?;
        assert!(root_json.contains(r#""TypeName""#));
        assert!(root_json.contains(r#""$ref": "MAP Core Schema-v0.0.7""#));
        assert!(!root_json.contains(r#""type_name""#));
        assert!(!root_json.contains(r#""InstanceTypeKind""#));
        assert!(root_json.contains(r#""meta""#));
        assert!(root_json.contains(r#""load_with""#));

        Ok(())
    }

    #[test]
    fn core_schema_json_compilation_emits_each_schema_holon_once() -> Result<()> {
        let out_dir = temp_out_dir();
        let compiled_files = compile_inputs(&[fixture_dir()], &out_dir)?;

        let mut map_core_schema_files = Vec::new();
        for path in compiled_files {
            let value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            let has_map_core_schema = value["holons"]
                .as_array()
                .map(|holons| holons.iter().any(|holon| holon["key"] == "MAP Core Schema-v0.0.7"))
                .unwrap_or(false);
            if has_map_core_schema {
                map_core_schema_files.push(path);
            }
        }

        assert_eq!(map_core_schema_files.len(), 1);
        assert!(map_core_schema_files[0].to_string_lossy().ends_with("core/root.json"));
        Ok(())
    }

    #[test]
    fn core_schema_generated_relationship_targets_use_canonical_ref_arrays() -> Result<()> {
        let out_dir = temp_out_dir();
        compile_inputs(&[fixture_dir()], &out_dir)?;

        let relationship_json =
            fs::read_to_string(out_dir.join("core/relationship-types.json"))?;
        let relationship_value: Value = serde_json::from_str(&relationship_json)?;
        let component_of = relationship_value["holons"]
            .as_array()
            .and_then(|holons| {
                holons.iter().find(|holon| {
                    holon["key"].as_str()
                        == Some("(TypeDescriptor)-[ComponentOf]->(Schema.HolonType)")
                })
            })
            .expect("ComponentOf relationship holon");
        let target = &component_of["relationships"]
            .as_array()
            .and_then(|relationships| {
                relationships
                    .iter()
                    .find(|relationship| relationship["name"].as_str() == Some("TargetType"))
            })
            .expect("TargetType relationship")["target"];
        assert!(target.is_array());
        assert_eq!(target[0]["$ref"], "Schema.HolonType");
        assert!(!relationship_json.contains(r##""$ref": "#TypeDescriptor.HolonType""##));

        Ok(())
    }

    #[test]
    fn core_schema_emits_one_normalized_has_inverse_target() -> Result<()> {
        let out_dir = temp_out_dir();
        compile_inputs(&[fixture_dir()], &out_dir)?;

        let relationship_json =
            fs::read_to_string(out_dir.join("core/relationship-types.json"))?;
        let relationship_value: Value = serde_json::from_str(&relationship_json)?;
        let instance_properties = relationship_value["holons"]
            .as_array()
            .and_then(|holons| {
                holons.iter().find(|holon| {
                    holon["key"].as_str()
                        == Some("(HolonType.TypeDescriptor)-[InstanceProperties]->(PropertyType.TypeDescriptor)")
                })
            })
            .expect("InstanceProperties relationship holon");
        let targets = &instance_properties["relationships"]
            .as_array()
            .and_then(|relationships| {
                relationships
                    .iter()
                    .find(|relationship| relationship["name"].as_str() == Some("HasInverse"))
            })
            .expect("HasInverse relationship")["target"];

        assert_eq!(targets.as_array().map(Vec::len), Some(1));
        assert_eq!(
            targets[0]["$ref"],
            "(PropertyType.TypeDescriptor)-[InstancePropertyFor]->(HolonType.TypeDescriptor)"
        );

        Ok(())
    }

    #[test]
    fn core_schema_lowers_command_affordances_as_relationships() -> Result<()> {
        let out_dir = temp_out_dir();
        compile_inputs(&[fixture_dir()], &out_dir)?;

        let commands_json = fs::read_to_string(out_dir.join("commands/schema.json"))?;
        let commands: Value = serde_json::from_str(&commands_json)?;
        let clone_holon = commands["holons"]
            .as_array()
            .and_then(|holons| {
                holons.iter().find(|holon| holon["key"].as_str() == Some("CloneHolon.CommandType"))
            })
            .expect("CloneHolon command descriptor");

        let affordance = clone_holon["relationships"]
            .as_array()
            .and_then(|relationships| {
                relationships.iter().find(|relationship| {
                    relationship["name"].as_str() == Some("CommandAffordedBy")
                })
            })
            .expect("CloneHolon CommandAffordedBy relationship");
        assert_eq!(affordance["target"], json!([{ "$ref": "HolonType.TypeDescriptor" }]));

        Ok(())
    }

    #[test]
    fn core_schema_leaves_max_cardinality_requiredness_to_the_meta_property_contract() -> Result<()>
    {
        let out_dir = temp_out_dir();
        compile_inputs(&[fixture_dir()], &out_dir)?;

        let property_json =
            fs::read_to_string(out_dir.join("core/property-types.json"))?;
        let property_value: Value = serde_json::from_str(&property_json)?;
        let max_cardinality = property_value["holons"]
            .as_array()
            .and_then(|holons| {
                holons
                    .iter()
                    .find(|holon| holon["key"].as_str() == Some("MaxCardinality.PropertyType"))
            })
            .expect("MaxCardinality property holon");

        assert!(max_cardinality["properties"].get("IsValueRequired").is_none());

        Ok(())
    }

    #[test]
    fn ordinary_keyword_injections_remain_keyword_driven_even_for_bootstrap_like_names(
    ) -> Result<()> {
        let input_dir = write_temp_tdl(
            "bootstrap-looking-property.tdl",
            r#"schema Example Schema-v0.0.1

abstract property MetaPropertyType {
  type MetaPropertyType.MetaTypeDescriptor
  value MapStringValueType.StringValueType
}
"#,
        )?;

        let out_dir = temp_out_dir();
        compile_inputs(&[input_dir], &out_dir)?;
        let compiled = fs::read_to_string(out_dir.join("bootstrap-looking-property.json"))?;
        let value: Value = serde_json::from_str(&compiled)?;
        let holon = value["holons"]
            .as_array()
            .and_then(|holons| holons.iter().find(|holon| holon["key"] == "MetaPropertyType"))
            .expect("MetaPropertyType holon");

        assert_eq!(holon["type"], "MetaPropertyType.MetaTypeDescriptor");
        assert_eq!(holon["properties"]["TypeName"], "MetaPropertyType");
        assert!(holon["relationships"].as_array().unwrap().iter().any(|relationship| {
            relationship["name"] == "ValueType"
                && relationship["target"][0]["$ref"] == "MapStringValueType.StringValueType"
        }));

        Ok(())
    }

    #[test]
    fn decompiler_keeps_bootstrap_anchors_out_of_ordinary_keyword_surface_forms() -> Result<()> {
        let out_dir = temp_out_dir();
        decompile_inputs(
            &[PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("generated")
                .join("json-imports")],
            &out_dir,
        )?;

        let root = fs::read_to_string(out_dir.join("core/root.tdl"))?;
        let abstract_values = fs::read_to_string(out_dir.join("core/abstract-value-types.tdl"))?;

        assert!(root.contains("holon \"MetaPropertyType.MetaTypeDescriptor\" {"));
        assert!(!root.contains("property \"MetaPropertyType.MetaTypeDescriptor\" {"));
        assert!(abstract_values.contains("holon \"MetaValueType.MetaTypeDescriptor\" {"));
        assert!(!abstract_values.contains("value \"MetaValueType.MetaTypeDescriptor\" {"));

        Ok(())
    }

    #[test]
    fn compile_rejects_duplicate_relative_paths_across_input_roots() -> Result<()> {
        let root_a = temp_tdl_dir().join("root-a");
        let root_b = temp_tdl_dir().join("root-b");
        let out_dir = temp_out_dir();
        let tdl = r#"schema Example Schema-v0.0.1

abstract value ExampleValueType
"#;

        write_tdl_file(&root_a.join("same.tdl"), tdl)?;
        write_tdl_file(&root_b.join("same.tdl"), tdl)?;

        let error = compile_inputs(&[root_a, root_b], &out_dir).expect_err("duplicate paths");
        assert!(error.to_string().contains("duplicate relative input path `same.tdl`"));

        Ok(())
    }
}
