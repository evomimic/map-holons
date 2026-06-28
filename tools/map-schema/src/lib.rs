use anyhow::{Context, Result};
pub mod diagnostics;
pub mod semantic;
pub mod symbols;

use crate::{
    diagnostics::Diagnostic,
    semantic::{
        push_reference, DescriptorHeader, Origin, ReferenceRole, RelationshipFlavor, Schema,
        SemanticModel, SemanticReference, SourceKind, TypeDescriptor,
    },
    symbols::SymbolTable,
};
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
enum JsonDescriptorKind {
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

/// Builds the derived semantic symbol table for JSON import inputs and returns a text dump.
///
/// This is a visibility/debugging aid only; the symbol table remains derived in-memory state and
/// should not be treated as a persisted source-of-truth artifact.
pub fn dump_symbols(inputs: &[PathBuf]) -> Result<String> {
    let files = collect_input_files(inputs)?;
    let parsed = parse_files(&files)?;
    let mut model = semantic_model_from_files(&parsed);
    let (symbols, diagnostics) = SymbolTable::from_model(&mut model);

    Ok(render_symbol_dump(&model, &symbols, &diagnostics))
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
    let mut model = semantic_model_from_file(file, schema_by_name);
    let _ = SymbolTable::from_model(&mut model);

    render_semantic_file(&model)
}

fn semantic_model_from_files(files: &[ParsedFile]) -> SemanticModel {
    let schema_by_name = schema_names_by_relative_path(files);
    let mut combined = SemanticModel::new();
    for file in files {
        let model = semantic_model_from_file(file, &schema_by_name);
        for schema in model.schemas {
            merge_schema(&mut combined, schema);
        }
        combined.descriptors.extend(model.descriptors);
    }
    combined
}

fn merge_schema(model: &mut SemanticModel, schema: Schema) {
    let Some(existing) = model.schemas.iter_mut().find(|candidate| candidate.key == schema.key)
    else {
        model.schemas.push(schema);
        return;
    };

    for dependency in schema.dependencies {
        if !existing
            .dependencies
            .iter()
            .any(|known| known.role == dependency.role && known.target == dependency.target)
        {
            existing.dependencies.push(dependency);
        }
    }

    if existing.header.is_none() {
        existing.header = schema.header;
    }
    existing.allows_additional_properties |= schema.allows_additional_properties;
    existing.allows_additional_relationships |= schema.allows_additional_relationships;
}

fn semantic_model_from_file(
    file: &ParsedFile,
    schema_by_name: &HashMap<String, String>,
) -> SemanticModel {
    let origin = Origin {
        source_kind: SourceKind::JsonImport,
        file_path: Some(file.relative_path.clone()),
        line: None,
        column: None,
    };
    let mut model = SemanticModel::new();
    let dependencies = schema_dependencies(file, schema_by_name)
        .into_iter()
        .map(|dependency| SemanticReference::unresolved(ReferenceRole::DependsOn, dependency))
        .collect::<Vec<_>>();
    let schema_holon =
        file.import.holons.iter().find(|holon| holon.descriptor_type == "Schema.HolonType");

    model.push_schema(Schema {
        name: file.schema_name.clone(),
        key: schema_holon
            .map(|holon| holon.key.clone())
            .unwrap_or_else(|| file.schema_name.clone()),
        origin: origin.clone(),
        dependencies,
        header: schema_holon.and_then(|holon| semantic_header(&holon.properties)),
        allows_additional_properties: schema_holon
            .map(|holon| bool_property(&holon.properties, "allows_additional_properties"))
            .unwrap_or(false),
        allows_additional_relationships: schema_holon
            .map(|holon| bool_property(&holon.properties, "allows_additional_relationships"))
            .unwrap_or(false),
    });

    for holon in &file.import.holons {
        if holon.descriptor_type == "Schema.HolonType" {
            continue;
        }
        model.push_descriptor(semantic_descriptor_from_holon(
            holon,
            &file.schema_name,
            origin.clone(),
        ));
    }

    model
}

fn semantic_descriptor_from_holon(
    holon: &HolonRecord,
    schema_name: &str,
    origin: Origin,
) -> TypeDescriptor {
    let kind = semantic_kind(holon);
    let mut descriptor =
        TypeDescriptor::new(holon.key.clone(), descriptor_name(holon), kind, schema_name, origin);
    descriptor.header = semantic_header(&holon.properties);
    descriptor.is_abstract = bool_property(&holon.properties, "is_abstract_type");
    descriptor.is_definitional = bool_property(&holon.properties, "is_definitional");
    descriptor.min_cardinality = integer_property(&holon.properties, "min_cardinality");
    descriptor.max_cardinality = integer_property(&holon.properties, "max_cardinality");
    descriptor.deletion_semantic = string_property(&holon.properties, "deletion_semantic");
    descriptor.is_ordered = bool_property(&holon.properties, "is_ordered");
    descriptor.allows_duplicates = bool_property(&holon.properties, "allows_duplicates");
    descriptor.allows_additional_properties =
        bool_property(&holon.properties, "allows_additional_properties");
    descriptor.allows_additional_relationships =
        bool_property(&holon.properties, "allows_additional_relationships");
    if matches!(classify(holon), JsonDescriptorKind::Relationship { inverse: true }) {
        descriptor.relationship_flavor = Some(RelationshipFlavor::Inverse);
    } else if kind == semantic::DescriptorKind::RelationshipType {
        descriptor.relationship_flavor = Some(RelationshipFlavor::Declared);
    }

    for relationship in &holon.relationships {
        let Some(role) = reference_role_for_relationship(&relationship.name) else {
            continue;
        };
        for target in target_strings(&relationship.target) {
            push_reference(&mut descriptor, SemanticReference::unresolved(role, target));
        }
    }
    descriptor
}

fn semantic_kind(holon: &HolonRecord) -> semantic::DescriptorKind {
    match classify(holon) {
        JsonDescriptorKind::Schema => semantic::DescriptorKind::Schema,
        JsonDescriptorKind::Value => semantic::DescriptorKind::ValueType,
        JsonDescriptorKind::Enum => semantic::DescriptorKind::Enum,
        JsonDescriptorKind::Property => semantic::DescriptorKind::PropertyType,
        JsonDescriptorKind::Relationship { .. } => semantic::DescriptorKind::RelationshipType,
        JsonDescriptorKind::Variant => semantic::DescriptorKind::EnumVariant,
        JsonDescriptorKind::Holon => {
            if descriptor_name(holon) == "TypeDescriptor" {
                semantic::DescriptorKind::TypeDescriptor
            } else {
                semantic::DescriptorKind::HolonType
            }
        }
    }
}

fn semantic_header(properties: &BTreeMap<String, Value>) -> Option<DescriptorHeader> {
    let header = DescriptorHeader {
        description: string_property(properties, "description"),
        display_name: string_property(properties, "display_name"),
        display_name_plural: string_property(properties, "display_name_plural"),
        type_name_plural: string_property(properties, "type_name_plural"),
    };
    if header.description.is_some()
        || header.display_name.is_some()
        || header.display_name_plural.is_some()
        || header.type_name_plural.is_some()
    {
        Some(header)
    } else {
        None
    }
}

fn reference_role_for_relationship(name: &str) -> Option<ReferenceRole> {
    match name {
        "ComponentOf" => Some(ReferenceRole::ComponentOf),
        "Extends" => Some(ReferenceRole::Extends),
        "UsesKeyRule" => Some(ReferenceRole::KeyRule),
        "SourceType" => Some(ReferenceRole::SourceType),
        "TargetType" => Some(ReferenceRole::TargetType),
        "InverseOf" => Some(ReferenceRole::InverseOf),
        "HasInverse" => Some(ReferenceRole::HasInverse),
        "ValueType" => Some(ReferenceRole::ValueType),
        "VariantOf" => Some(ReferenceRole::VariantOf),
        "InstanceProperties" => Some(ReferenceRole::InstanceProperty),
        "InstanceRelationships" => Some(ReferenceRole::InstanceRelationship),
        _ => None,
    }
}

fn render_semantic_file(model: &SemanticModel) -> Result<String> {
    let mut out = String::new();
    let schema = model.schemas.first().context("semantic model has no schema")?;
    render_semantic_schema_decl(&mut out, schema);

    let enum_variant_groups = semantic_enum_variant_groups(model);
    let mut first_descriptor = true;
    for descriptor in &model.descriptors {
        if enum_variant_groups
            .values()
            .any(|variants| variants.iter().any(|variant| variant.key == descriptor.key))
        {
            continue;
        }
        if !first_descriptor {
            out.push('\n');
        }
        first_descriptor = false;
        render_semantic_descriptor(&mut out, descriptor, &enum_variant_groups)?;
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

fn render_semantic_schema_decl(out: &mut String, schema: &Schema) {
    let has_body = !schema.dependencies.is_empty()
        || schema.header.is_some()
        || schema.allows_additional_properties
        || schema.allows_additional_relationships;
    if !has_body {
        out.push_str(&format!("schema {}\n", schema.name));
        return;
    }

    out.push_str(&format!("schema {} {{\n", schema.name));
    for dependency in &schema.dependencies {
        out.push_str(&format!("{}depends_on {}\n", INDENT, dependency.target));
    }
    if let Some(header) = &schema.header {
        render_semantic_header(out, 1, header);
    }
    if schema.allows_additional_properties {
        out.push_str(&format!("{}allows_additional_properties\n", INDENT));
    }
    if schema.allows_additional_relationships {
        out.push_str(&format!("{}allows_additional_relationships\n", INDENT));
    }
    out.push_str("}\n");
}

fn semantic_enum_variant_groups<'a>(
    model: &'a SemanticModel,
) -> HashMap<String, Vec<&'a TypeDescriptor>> {
    let mut groups: HashMap<String, Vec<&TypeDescriptor>> = HashMap::new();
    for descriptor in &model.descriptors {
        if descriptor.kind == semantic::DescriptorKind::EnumVariant {
            if let Some(variant_of) = &descriptor.variant_of {
                groups.entry(variant_of.target.clone()).or_default().push(descriptor);
            }
        }
    }
    groups
}

fn render_semantic_descriptor(
    out: &mut String,
    descriptor: &TypeDescriptor,
    enum_variant_groups: &HashMap<String, Vec<&TypeDescriptor>>,
) -> Result<()> {
    match descriptor.kind {
        semantic::DescriptorKind::ValueType => render_semantic_value(out, descriptor),
        semantic::DescriptorKind::Enum => {
            render_semantic_enum(out, descriptor, enum_variant_groups)
        }
        semantic::DescriptorKind::PropertyType => render_semantic_property(out, descriptor),
        semantic::DescriptorKind::RelationshipType => render_semantic_relationship(out, descriptor),
        semantic::DescriptorKind::EnumVariant => render_semantic_variant(out, descriptor),
        semantic::DescriptorKind::HolonType | semantic::DescriptorKind::TypeDescriptor => {
            render_semantic_holon(out, descriptor)
        }
        semantic::DescriptorKind::Schema => Ok(()),
    }
}

fn render_semantic_value(out: &mut String, descriptor: &TypeDescriptor) -> Result<()> {
    let head = descriptor_head("value", descriptor);
    let mut clauses = Vec::new();
    if let Some(parent) = &descriptor.extends {
        if parent.target != "ValueType" {
            clauses.push(format!("extends {}", parent.target));
        }
    }
    append_semantic_body(out, &head, &clauses, descriptor.header.as_ref())
}

fn render_semantic_enum(
    out: &mut String,
    descriptor: &TypeDescriptor,
    enum_variant_groups: &HashMap<String, Vec<&TypeDescriptor>>,
) -> Result<()> {
    let head = descriptor_head("enum", descriptor);
    let mut clauses = Vec::new();
    if let Some(parent) = &descriptor.extends {
        if parent.target != "ValueType" {
            clauses.push(format!("extends {}", parent.target));
        }
    }
    let mut body_lines = Vec::new();
    if let Some(header) = &descriptor.header {
        body_lines.extend(semantic_header_lines(header));
    }
    if let Some(variants) = enum_variant_groups.get(&descriptor.name) {
        body_lines.push("variants {".to_string());
        for variant in variants {
            let rendered = semantic_variant_declaration(variant);
            body_lines.extend(rendered.lines().map(|line| format!("{}{}", INDENT, line)));
        }
        body_lines.push("}".to_string());
    }
    append_semantic_body_with_lines(out, &head, &clauses, &body_lines)
}

fn render_semantic_property(out: &mut String, descriptor: &TypeDescriptor) -> Result<()> {
    let head = descriptor_head("property", descriptor);
    let mut clauses = Vec::new();
    if let Some(value_type) = &descriptor.value_type {
        clauses.push(format!("value {}", value_type.target));
    }
    if let Some(parent) = &descriptor.extends {
        if parent.target != "PropertyType" {
            clauses.push(format!("extends {}", parent.target));
        }
    }
    append_semantic_body(out, &head, &clauses, descriptor.header.as_ref())
}

fn render_semantic_relationship(out: &mut String, descriptor: &TypeDescriptor) -> Result<()> {
    let keyword = match descriptor.relationship_flavor {
        Some(RelationshipFlavor::Inverse) => "inverse relationship",
        _ if descriptor.is_definitional => "def relationship",
        _ => "relationship",
    };
    let head = descriptor_head(keyword, descriptor);
    let mut clauses = Vec::new();
    if let Some(source) = &descriptor.source_type {
        clauses.push(format!("source {}", source.target));
    }
    if let Some(target) = &descriptor.target_type {
        clauses.push(format!("target {}", target.target));
    }
    if let Some(inverse_of) = &descriptor.inverse_of {
        clauses.push(format!("inverse {}", relationship_label(&inverse_of.target)));
    }
    if let Some(key_rule) = &descriptor.key_rule {
        clauses.push(format!("keyrule {}", key_rule.target));
    }
    if let (Some(min), Some(max)) = (descriptor.min_cardinality, descriptor.max_cardinality) {
        clauses.push(format!("cardinality {}..{}", min, max));
    }
    if descriptor.is_ordered {
        clauses.push("ordered".to_string());
    }
    if descriptor.allows_duplicates {
        clauses.push("duplicates".to_string());
    }
    if let Some(deletion_semantic) = &descriptor.deletion_semantic {
        clauses.push(format!("deletion_semantic {}", deletion_semantic));
    }
    append_semantic_body(out, &head, &clauses, descriptor.header.as_ref())
}

fn render_semantic_variant(out: &mut String, descriptor: &TypeDescriptor) -> Result<()> {
    out.push_str(&semantic_variant_declaration(descriptor));
    Ok(())
}

fn render_semantic_holon(out: &mut String, descriptor: &TypeDescriptor) -> Result<()> {
    let head = descriptor_head("holon", descriptor);
    let mut clauses = Vec::new();
    if let Some(parent) = &descriptor.extends {
        if parent.target != "HolonType" {
            clauses.push(format!("extends {}", parent.target));
        }
    }
    if descriptor.allows_additional_properties {
        clauses.push("allows_additional_properties".to_string());
    }
    if descriptor.allows_additional_relationships {
        clauses.push("allows_additional_relationships".to_string());
    }
    let mut body_lines = Vec::new();
    if let Some(header) = &descriptor.header {
        body_lines.extend(semantic_header_lines(header));
    }
    if !descriptor.instance_properties.is_empty() {
        body_lines.push("properties {".to_string());
        for property in &descriptor.instance_properties {
            body_lines.push(format!("{}{}", INDENT, property.target));
        }
        body_lines.push("}".to_string());
    }
    if !descriptor.instance_relationships.is_empty() {
        body_lines.push("relationships {".to_string());
        for relationship in &descriptor.instance_relationships {
            body_lines.push(format!("{}{}", INDENT, relationship_ref(&relationship.target)));
        }
        body_lines.push("}".to_string());
    }
    append_semantic_body_with_lines(out, &head, &clauses, &body_lines)
}

fn descriptor_head(keyword: &str, descriptor: &TypeDescriptor) -> String {
    let mut head = String::new();
    if descriptor.is_abstract {
        head.push_str("abstract ");
    }
    head.push_str(keyword);
    head.push(' ');
    head.push_str(&descriptor.name);
    head
}

fn append_semantic_body(
    out: &mut String,
    head: &str,
    clauses: &[String],
    header: Option<&DescriptorHeader>,
) -> Result<()> {
    let body_lines = header.map(semantic_header_lines).unwrap_or_default();
    append_semantic_body_with_lines(out, head, clauses, &body_lines)
}

fn append_semantic_body_with_lines(
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

fn semantic_variant_declaration(descriptor: &TypeDescriptor) -> String {
    let name =
        descriptor.name.rsplit_once('.').map(|(_, suffix)| suffix).unwrap_or(&descriptor.name);
    let Some(header) = &descriptor.header else {
        return format!("variant {}\n", name);
    };
    let mut out = String::new();
    out.push_str(&format!("variant {} {{\n", name));
    for line in semantic_header_lines(header) {
        out.push_str(&format!("{}{}\n", INDENT, line));
    }
    out.push_str("}\n");
    out
}

fn render_semantic_header(out: &mut String, indent_level: usize, header: &DescriptorHeader) {
    for line in semantic_header_lines(header) {
        out.push_str(&format!("{}{}\n", INDENT.repeat(indent_level), line));
    }
}

fn semantic_header_lines(header: &DescriptorHeader) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("header {".to_string());
    if let Some(description) = &header.description {
        lines.push(format!("{}description: {}", INDENT, json_literal(description)));
    }
    if let Some(display_name) = &header.display_name {
        lines.push(format!("{}display_name: {}", INDENT, json_literal(display_name)));
    }
    if let Some(display_plural) = &header.display_name_plural {
        lines.push(format!("{}display_plural: {}", INDENT, json_literal(display_plural)));
    }
    if let Some(plural) = &header.type_name_plural {
        lines.push(format!("{}plural: {}", INDENT, json_literal(plural)));
    }
    lines.push("}".to_string());
    lines
}

fn render_symbol_dump(
    model: &SemanticModel,
    symbols: &SymbolTable,
    diagnostics: &[Diagnostic],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("symbols: {}\n", symbols.symbols().len()));
    out.push_str(&format!("schemas: {}\n", model.schemas.len()));
    out.push_str(&format!("descriptors: {}\n", model.descriptors.len()));
    out.push_str(&format!("diagnostics: {}\n\n", diagnostics.len()));

    out.push_str("symbol table\n");
    for symbol in symbols.symbols() {
        out.push_str(&format!(
            "  #{:04} {:?} key={} name={}",
            symbol.id.0, symbol.kind, symbol.key, symbol.name
        ));
        if let Some(schema) = &symbol.owning_schema {
            out.push_str(&format!(" schema={schema}"));
        }
        out.push_str(&format!(" origin={}\n", format_origin(&symbol.origin)));
    }

    let unresolved = symbols.collect_unresolved_references(model);
    if !unresolved.is_empty() {
        out.push_str("\nunresolved references\n");
        for reference in unresolved {
            out.push_str(&format!("  {:?} -> {}\n", reference.role, reference.target));
        }
    }

    if !diagnostics.is_empty() {
        out.push_str("\ndiagnostics\n");
        for diagnostic in diagnostics {
            out.push_str(&format!(
                "  {:?} {:?} origin={}\n",
                diagnostic.severity,
                diagnostic.kind,
                diagnostic
                    .origin
                    .as_ref()
                    .map(format_origin)
                    .unwrap_or_else(|| "<unknown>".to_string())
            ));
        }
    }

    out
}

fn format_origin(origin: &Origin) -> String {
    let source = match origin.source_kind {
        SourceKind::JsonImport => "json",
        SourceKind::TdlSource => "tdl",
        SourceKind::Generated => "generated",
    };
    let path = origin
        .file_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<none>".to_string());
    match (origin.line, origin.column) {
        (Some(line), Some(column)) => format!("{source}:{path}:{line}:{column}"),
        (Some(line), None) => format!("{source}:{path}:{line}"),
        _ => format!("{source}:{path}"),
    }
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

fn classify(holon: &HolonRecord) -> JsonDescriptorKind {
    if holon.descriptor_type == "Schema.HolonType" {
        return JsonDescriptorKind::Schema;
    }

    if has_relationship(holon, "SourceType") && has_relationship(holon, "TargetType") {
        return JsonDescriptorKind::Relationship { inverse: has_relationship(holon, "InverseOf") };
    }

    if has_relationship(holon, "ValueType") || descriptor_name(holon).ends_with("PropertyType") {
        return JsonDescriptorKind::Property;
    }

    match holon.properties.get("instance_type_kind").and_then(Value::as_str).unwrap_or("") {
        "TypeKind.Value.Enum" => JsonDescriptorKind::Enum,
        "TypeKind.EnumVariant" => JsonDescriptorKind::Variant,
        kind if kind.starts_with("TypeKind.Value.") => JsonDescriptorKind::Value,
        _ if descriptor_name(holon).ends_with("ValueType") => JsonDescriptorKind::Value,
        _ => JsonDescriptorKind::Holon,
    }
}

fn descriptor_name(holon: &HolonRecord) -> String {
    string_property(&holon.properties, "type_name").unwrap_or_else(|| holon.key.clone())
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
