use anyhow::{Context, Result};
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
    meta: ImportMeta,
    holons: Vec<HolonRecord>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ImportMeta {
    #[serde(default)]
    load_with: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct HolonRecord {
    key: String,
    #[serde(rename = "type")]
    descriptor_type: String,
    #[serde(default)]
    properties: BTreeMap<String, Value>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DescriptorKind {
    Schema,
    Value,
    Enum,
    Property,
    Relationship { inverse: bool },
    Variant,
    Holon,
}

pub fn decompile_inputs(inputs: &[PathBuf], out_dir: &Path) -> Result<Vec<PathBuf>> {
    let files = collect_input_files(inputs)?;
    let parsed = parse_files(&files)?;
    let schema_by_name = schema_names_by_relative_path(&parsed);
    let mut written = Vec::new();

    for file in &parsed {
        let output = out_dir.join(file.relative_path.with_extension("tdl"));
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }

        let contents = render_file(file, &schema_by_name)?;
        fs::write(&output, contents)
            .with_context(|| format!("writing decompiled TDL to {}", output.display()))?;
        written.push(output);
    }

    Ok(written)
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
        let import: ImportFile = serde_json::from_str(&raw)
            .with_context(|| format!("parsing JSON import file {}", path.display()))?;
        let schema_name = infer_schema_name(&import)
            .with_context(|| format!("inferring schema name for {}", path.display()))?;
        parsed.push(ParsedFile {
            relative_path: discovered_file.relative_path.clone(),
            schema_name,
            import,
        });
    }

    Ok(parsed)
}

fn schema_names_by_relative_path(parsed: &[ParsedFile]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for file in parsed {
        if let Some(file_name) = file.relative_path.file_name().and_then(|name| name.to_str()) {
            map.insert(file_name.to_string(), file.schema_name.clone());
        }
    }
    map
}

fn infer_schema_name(import: &ImportFile) -> Option<String> {
    import
        .holons
        .iter()
        .find(|holon| holon.descriptor_type == "Schema.HolonType")
        .and_then(schema_name_from_holon)
        .or_else(|| import.holons.first().and_then(component_of_schema_name))
}

fn schema_name_from_holon(holon: &HolonRecord) -> Option<String> {
    string_property(&holon.properties, "schema_name").or_else(|| component_of_schema_name(holon))
}

fn component_of_schema_name(holon: &HolonRecord) -> Option<String> {
    relationship_targets(holon, "ComponentOf").into_iter().next()
}

fn render_file(file: &ParsedFile, schema_by_name: &HashMap<String, String>) -> Result<String> {
    let mut out = String::new();
    let dependencies = schema_dependencies(file, schema_by_name);
    let schema_holon =
        file.import.holons.iter().find(|holon| holon.descriptor_type == "Schema.HolonType");

    render_schema_decl(&mut out, &file.schema_name, schema_holon, &dependencies)?;

    let enum_variant_groups = group_enum_variants(&file.import.holons);
    let mut first_descriptor = true;

    for holon in &file.import.holons {
        if holon.descriptor_type == "Schema.HolonType" {
            continue;
        }

        if is_grouped_variant(holon, &enum_variant_groups) {
            continue;
        }

        if !first_descriptor {
            out.push_str("\n");
        }
        first_descriptor = false;

        render_descriptor(&mut out, holon, &enum_variant_groups)?;
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
    }

    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');

    Ok(out)
}

fn schema_dependencies(file: &ParsedFile, schema_by_name: &HashMap<String, String>) -> Vec<String> {
    let mut deps = Vec::new();
    let mut seen = HashSet::new();
    for referenced in &file.import.meta.load_with {
        let Some(schema_name) = schema_by_name.get(referenced) else {
            continue;
        };
        if schema_name != &file.schema_name && seen.insert(schema_name.clone()) {
            deps.push(schema_name.clone());
        }
    }
    deps
}

fn render_schema_decl(
    out: &mut String,
    schema_name: &str,
    schema_holon: Option<&HolonRecord>,
    dependencies: &[String],
) -> Result<()> {
    let has_body = schema_holon
        .map(|holon| has_schema_body(holon) || !dependencies.is_empty())
        .unwrap_or(!dependencies.is_empty());

    if !has_body {
        out.push_str(&format!("schema {}\n", schema_name));
        return Ok(());
    }

    out.push_str(&format!("schema {} {{\n", schema_name));
    for dep in dependencies {
        out.push_str(&format!("{}depends_on {}\n", INDENT, dep));
    }
    if let Some(holon) = schema_holon {
        if let Some(header) = render_header_block(&holon.properties) {
            render_block(out, 1, &header);
        }
        if bool_property(&holon.properties, "allows_additional_properties") {
            out.push_str(&format!("{}allows_additional_properties\n", INDENT));
        }
        if bool_property(&holon.properties, "allows_additional_relationships") {
            out.push_str(&format!("{}allows_additional_relationships\n", INDENT));
        }
    }
    out.push_str("}\n");
    Ok(())
}

fn has_schema_body(holon: &HolonRecord) -> bool {
    render_header_block(&holon.properties).is_some()
        || bool_property(&holon.properties, "allows_additional_properties")
        || bool_property(&holon.properties, "allows_additional_relationships")
}

fn group_enum_variants<'a>(holons: &'a [HolonRecord]) -> HashMap<String, Vec<&'a HolonRecord>> {
    let mut groups: HashMap<String, Vec<&HolonRecord>> = HashMap::new();
    for holon in holons {
        if matches!(classify(holon), DescriptorKind::Variant) {
            if let Some(enum_name) = variant_of(holon) {
                groups.entry(enum_name).or_default().push(holon);
            }
        }
    }
    groups
}

fn is_grouped_variant(holon: &HolonRecord, groups: &HashMap<String, Vec<&HolonRecord>>) -> bool {
    variant_of(holon)
        .map(|enum_name| groups.get(&enum_name).is_some())
        .unwrap_or(false)
        && has_variant_of(holon)
}

fn render_descriptor(
    out: &mut String,
    holon: &HolonRecord,
    enum_variant_groups: &HashMap<String, Vec<&HolonRecord>>,
) -> Result<()> {
    match classify(holon) {
        DescriptorKind::Value => render_value(out, holon),
        DescriptorKind::Enum => render_enum(out, holon, enum_variant_groups),
        DescriptorKind::Property => render_property(out, holon),
        DescriptorKind::Relationship { inverse } => render_relationship(out, holon, inverse),
        DescriptorKind::Variant => render_variant(out, holon),
        DescriptorKind::Holon => render_holon(out, holon),
        DescriptorKind::Schema => Ok(()),
    }
}

fn render_value(out: &mut String, holon: &HolonRecord) -> Result<()> {
    let abstract_flag = bool_property(&holon.properties, "is_abstract_type");
    let mut line = String::new();
    if abstract_flag {
        line.push_str("abstract ");
    }
    line.push_str(&format!("value {}", descriptor_name(holon)));
    let mut clauses = Vec::new();
    if let Some(parent) = extends_target(holon) {
        if parent != "ValueType" {
            clauses.push(format!("extends {}", parent));
        }
    }
    append_descriptor_body(out, &line, &clauses, render_header_block(&holon.properties).as_deref())?;
    Ok(())
}

fn render_enum(
    out: &mut String,
    holon: &HolonRecord,
    enum_variant_groups: &HashMap<String, Vec<&HolonRecord>>,
) -> Result<()> {
    let abstract_flag = bool_property(&holon.properties, "is_abstract_type");
    let mut line = String::new();
    if abstract_flag {
        line.push_str("abstract ");
    }
    line.push_str(&format!("enum {}", descriptor_name(holon)));
    let mut clauses = Vec::new();
    if let Some(parent) = extends_target(holon) {
        if parent != "ValueType" {
            clauses.push(format!("extends {}", parent));
        }
    }
    let mut body_lines = Vec::new();
    if let Some(header) = render_header_block(&holon.properties) {
        body_lines.extend(header);
    }
    if let Some(variants) = enum_variant_groups.get(&descriptor_name(holon)) {
        body_lines.push("variants {".to_string());
        for variant in variants {
            let rendered = render_variant_declaration(variant)?;
            body_lines.extend(rendered.lines().map(|line| format!("{}{}", INDENT, line)));
        }
        body_lines.push("}".to_string());
    }
    append_descriptor_body_with_prebuilt(out, &line, &clauses, &body_lines)?;
    Ok(())
}

fn render_property(out: &mut String, holon: &HolonRecord) -> Result<()> {
    let abstract_flag = bool_property(&holon.properties, "is_abstract_type");
    let mut line = String::new();
    if abstract_flag {
        line.push_str("abstract ");
    }
    line.push_str(&format!("property {}", descriptor_name(holon)));
    let mut clauses = Vec::new();
    if let Some(value_type) = relationship_targets(holon, "ValueType").into_iter().next() {
        clauses.push(format!("value {}", value_type));
    }
    if let Some(parent) = extends_target(holon) {
        if parent != "PropertyType" {
            clauses.push(format!("extends {}", parent));
        }
    }
    append_descriptor_body(out, &line, &clauses, render_header_block(&holon.properties).as_deref())?;
    Ok(())
}

fn render_relationship(out: &mut String, holon: &HolonRecord, inverse: bool) -> Result<()> {
    let abstract_flag = bool_property(&holon.properties, "is_abstract_type");
    let mut line = String::new();
    if abstract_flag {
        line.push_str("abstract ");
    }
    if inverse {
        line.push_str("inverse relationship ");
    } else if bool_property(&holon.properties, "is_definitional") {
        line.push_str("def relationship ");
    } else {
        line.push_str("relationship ");
    }
    line.push_str(&descriptor_name(holon));

    let mut clauses = Vec::new();
    if let Some(source) = relationship_targets(holon, "SourceType").into_iter().next() {
        clauses.push(format!("source {}", source));
    }
    if let Some(target) = relationship_targets(holon, "TargetType").into_iter().next() {
        clauses.push(format!("target {}", target));
    }
    if let Some(inverse_of) = relationship_targets(holon, "InverseOf").into_iter().next() {
        clauses.push(format!("inverse {}", relationship_label(&inverse_of)));
    }
    if let Some(keyrule) = relationship_targets(holon, "UsesKeyRule").into_iter().next() {
        clauses.push(format!("keyrule {}", keyrule));
    }
    if let (Some(min), Some(max)) = (
        integer_property(&holon.properties, "min_cardinality"),
        integer_property(&holon.properties, "max_cardinality"),
    ) {
        clauses.push(format!("cardinality {}..{}", min, max));
    }
    if bool_property(&holon.properties, "is_ordered") {
        clauses.push("ordered".to_string());
    }
    if bool_property(&holon.properties, "allows_duplicates") {
        clauses.push("duplicates".to_string());
    }
    if let Some(deletion) = string_property(&holon.properties, "deletion_semantic") {
        clauses.push(format!("deletion_semantic {}", deletion));
    }
    append_descriptor_body(out, &line, &clauses, render_header_block(&holon.properties).as_deref())?;
    Ok(())
}

fn render_variant(out: &mut String, holon: &HolonRecord) -> Result<()> {
    let rendered = render_variant_declaration(holon)?;
    out.push_str(&rendered);
    Ok(())
}

fn render_holon(out: &mut String, holon: &HolonRecord) -> Result<()> {
    let abstract_flag = bool_property(&holon.properties, "is_abstract_type");
    let mut line = String::new();
    if abstract_flag {
        line.push_str("abstract ");
    }
    line.push_str(&format!("holon {}", descriptor_name(holon)));
    let mut clauses = Vec::new();
    if let Some(parent) = extends_target(holon) {
        if parent != "HolonType" {
            clauses.push(format!("extends {}", parent));
        }
    }
    if bool_property(&holon.properties, "allows_additional_properties") {
        clauses.push("allows_additional_properties".to_string());
    }
    if bool_property(&holon.properties, "allows_additional_relationships") {
        clauses.push("allows_additional_relationships".to_string());
    }
    let mut body_lines = Vec::new();
    if let Some(header) = render_header_block(&holon.properties) {
        body_lines.extend(header);
    }
    let properties = relationship_targets(holon, "InstanceProperties");
    if !properties.is_empty() {
        body_lines.push("properties {".to_string());
        for property in properties {
            body_lines.push(format!("{}{}", INDENT, property));
        }
        body_lines.push("}".to_string());
    }
    let relationships = relationship_targets(holon, "InstanceRelationships");
    if !relationships.is_empty() {
        body_lines.push("relationships {".to_string());
        for relationship in relationships {
            body_lines.push(format!("{}{}", INDENT, relationship_ref(&relationship)));
        }
        body_lines.push("}".to_string());
    }
    append_descriptor_body_with_prebuilt(out, &line, &clauses, &body_lines)?;
    Ok(())
}

fn render_variant_declaration(holon: &HolonRecord) -> Result<String> {
    let mut lines = Vec::new();
    let name = variant_name(holon);
    lines.push(format!("variant {}", name));
    if let Some(header) = render_header_block(&holon.properties) {
        lines.extend(header);
    }
    let rendered = if lines.len() == 1 {
        format!("variant {}\n", name)
    } else {
        let mut out = String::new();
        out.push_str(&format!("variant {} {{\n", name));
        for line in lines.iter().skip(1) {
            out.push_str(&format!("{}{}\n", INDENT, line));
        }
        out.push_str("}\n");
        out
    };
    Ok(rendered)
}

fn append_descriptor_body(
    out: &mut String,
    head: &str,
    clauses: &[String],
    header: Option<&[String]>,
) -> Result<()> {
    if clauses.is_empty() && header.is_none() {
        out.push_str(head);
        out.push('\n');
        return Ok(());
    }

    out.push_str(head);
    out.push_str(" {\n");
    for clause in clauses {
        out.push_str(&format!("{}{}\n", INDENT, clause));
    }
    if let Some(header) = header {
        for line in header {
            out.push_str(&format!("{}{}\n", INDENT, line));
        }
    }
    out.push_str("}\n");
    Ok(())
}

fn append_descriptor_body_with_prebuilt(
    out: &mut String,
    head: &str,
    clauses: &[String],
    body_lines: &[String],
) -> Result<()> {
    if clauses.is_empty() && body_lines.is_empty() {
        out.push_str(head);
        out.push('\n');
        return Ok(());
    }

    out.push_str(head);
    out.push_str(" {\n");
    for clause in clauses {
        out.push_str(&format!("{}{}\n", INDENT, clause));
    }
    for line in body_lines {
        out.push_str(&format!("{}{}\n", INDENT, line));
    }
    out.push_str("}\n");
    Ok(())
}

fn render_block(out: &mut String, indent_level: usize, block_lines: &[String]) {
    for line in block_lines {
        out.push_str(&format!("{}{}\n", INDENT.repeat(indent_level), line));
    }
}

fn render_header_block(properties: &BTreeMap<String, Value>) -> Option<Vec<String>> {
    let mut lines = Vec::new();
    if let Some(description) = string_property(properties, "description") {
        lines.push("header {".to_string());
        lines.push(format!("{}description: {}", INDENT, json_literal(&description)));
        push_optional_header_field(
            &mut lines,
            "display_name",
            string_property(properties, "display_name"),
        );
        push_optional_header_field(
            &mut lines,
            "display_plural",
            string_property(properties, "display_name_plural"),
        );
        push_optional_header_field(&mut lines, "plural", string_property(properties, "type_name_plural"));
        lines.push("}".to_string());
    } else if let Some(display_name) = string_property(properties, "display_name") {
        lines.push("header {".to_string());
        lines.push(format!("{}display_name: {}", INDENT, json_literal(&display_name)));
        push_optional_header_field(
            &mut lines,
            "display_plural",
            string_property(properties, "display_name_plural"),
        );
        push_optional_header_field(&mut lines, "plural", string_property(properties, "type_name_plural"));
        lines.push("}".to_string());
    } else {
        return None;
    }
    Some(lines)
}

fn push_optional_header_field(lines: &mut Vec<String>, field: &str, value: Option<String>) {
    if let Some(value) = value {
        lines.push(format!("{}{}: {}", INDENT, field, json_literal(&value)));
    }
}

fn classify(holon: &HolonRecord) -> DescriptorKind {
    if holon.descriptor_type == "Schema.HolonType" {
        return DescriptorKind::Schema;
    }

    if has_relationship(holon, "SourceType") && has_relationship(holon, "TargetType") {
        return DescriptorKind::Relationship { inverse: has_relationship(holon, "InverseOf") };
    }

    if has_relationship(holon, "ValueType") || descriptor_name(holon).ends_with("PropertyType") {
        return DescriptorKind::Property;
    }

    match holon
        .properties
        .get("instance_type_kind")
        .and_then(Value::as_str)
        .unwrap_or("")
    {
        "TypeKind.Value.Enum" => DescriptorKind::Enum,
        "TypeKind.EnumVariant" => DescriptorKind::Variant,
        kind if kind.starts_with("TypeKind.Value.") => DescriptorKind::Value,
        _ if descriptor_name(holon).ends_with("ValueType") => DescriptorKind::Value,
        _ => DescriptorKind::Holon,
    }
}

fn descriptor_name(holon: &HolonRecord) -> String {
    string_property(&holon.properties, "type_name").unwrap_or_else(|| holon.key.clone())
}

fn variant_name(holon: &HolonRecord) -> String {
    if let Some(name) = string_property(&holon.properties, "type_name") {
        if let Some((_, suffix)) = name.rsplit_once('.') {
            return suffix.to_string();
        }
        return name;
    }
    holon
        .key
        .rsplit_once('.')
        .map(|(_, suffix)| suffix.to_string())
        .unwrap_or_else(|| holon.key.clone())
}

fn has_relationship(holon: &HolonRecord, name: &str) -> bool {
    holon.relationships.iter().any(|relationship| relationship.name == name)
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

fn relationship_label(reference: &str) -> String {
    if let Some(start) = reference.find("-[") {
        if let Some(end) = reference[start + 2..].find("]->") {
            return reference[start + 2..start + 2 + end].to_string();
        }
    }
    reference.to_string()
}

fn relationship_ref(reference: &str) -> String {
    if reference.contains("-[") {
        reference.to_string()
    } else {
        reference.to_string()
    }
}

fn variant_of(holon: &HolonRecord) -> Option<String> {
    relationship_targets(holon, "VariantOf").into_iter().next()
}

fn has_variant_of(holon: &HolonRecord) -> bool {
    has_relationship(holon, "VariantOf")
}

fn extends_target(holon: &HolonRecord) -> Option<String> {
    relationship_targets(holon, "Extends").into_iter().next()
}

fn bool_property(properties: &BTreeMap<String, Value>, key: &str) -> bool {
    properties.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn integer_property(properties: &BTreeMap<String, Value>, key: &str) -> Option<i64> {
    properties.get(key).and_then(Value::as_i64)
}

fn string_property(properties: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    properties.get(key).and_then(Value::as_str).map(ToString::to_string)
}

fn json_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{}\"", value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("host")
            .join("import_files")
            .join("map-schema")
            .join("core-schema")
    }

    fn temp_out_dir() -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("map-schema-decompile-{nanos}"))
    }

    #[test]
    fn decompiles_core_schema_corpus() -> Result<()> {
        let out_dir = temp_out_dir();
        let files = decompile_inputs(&[fixture_dir()], &out_dir)?;
        assert_eq!(files.len(), 11);

        let root = fs::read_to_string(out_dir.join("MAP Schema Types-map-core-schema-root.tdl"))?;
        assert!(root.contains("schema MAP Core Schema-v0.0.7"));
        assert!(root.contains("holon TypeDescriptor"));
        assert!(root.contains("abstract holon HolonType"));

        let property = fs::read_to_string(
            out_dir.join("MAP Schema Types-map-core-schema-property-types.tdl"),
        )?;
        assert!(property.contains("property Description"));
        assert!(property.contains("value MapStringValueType"));

        let relationship = fs::read_to_string(
            out_dir.join("MAP Schema Types-map-core-schema-relationship-types.tdl"),
        )?;
        assert!(relationship.contains("def relationship ComponentOf"));
        assert!(relationship.contains("source TypeDescriptor.HolonType"));
        assert!(relationship.contains("target Schema.HolonType"));

        let concrete = fs::read_to_string(
            out_dir.join("MAP Schema Types-map-core-schema-concrete-value-types.tdl"),
        )?;
        assert!(concrete.contains("enum TypeKind"));
        assert!(concrete.contains("variant Holon"));
        assert!(concrete.contains("variant Property"));

        Ok(())
    }
}
