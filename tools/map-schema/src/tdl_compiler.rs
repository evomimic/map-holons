use crate::{
    diagnostics::{format_diagnostics, Diagnostic},
    literal_bridge::json_value_to_literal,
    loader_ir::{LoaderDocument, LoaderMeta},
    schema_index::SymbolIndex,
    schema_ir::{
        DescriptorHeader, DescriptorKind, LiteralRelationship, Origin, ReferenceRole,
        RelationshipFlavor, Schema, SemanticModel, SemanticReference, SourceKind, TypeDescriptor,
    },
    schema_to_loader_ir::{
        build_emitted_key_lookup, emit_loader_document_json, lower_schema_model_to_loader_ir,
    },
};
use anyhow::{anyhow, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const GENERATOR_NAME: &str = "MAP Schema Compiler";
const DEFAULT_VALUE_EXTENDS: &str = "ValueType";
const DEFAULT_ENUM_EXTENDS: &str = "MapEnumValueType";
const DEFAULT_ENUM_VARIANT_EXTENDS: &str = "MapEnumVariantValueType";
const DEFAULT_PROPERTY_EXTENDS: &str = "PropertyType";
const DEFAULT_DECLARED_RELATIONSHIP_EXTENDS: &str = "DeclaredRelationshipType";
const DEFAULT_INVERSE_RELATIONSHIP_EXTENDS: &str = "InverseRelationshipType";
const DEFAULT_VARIANT_EXTENDS: &str = "ValueType";

#[derive(Debug, Clone)]
struct DiscoveredFile {
    source_path: PathBuf,
    relative_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ParsedTdlFile {
    relative_path: PathBuf,
    schema: TdlSchema,
    descriptors: Vec<TdlDescriptor>,
}

#[derive(Debug, Clone)]
struct TdlSchema {
    name: String,
    origin: Origin,
    dependencies: Vec<String>,
    literal_properties: map_schema_semantic::LiteralObject,
    literal_relationships: Vec<LiteralRelationship>,
    header: Option<DescriptorHeader>,
    allows_additional_properties: bool,
    allows_additional_relationships: bool,
}

#[derive(Debug, Clone)]
struct TdlDescriptor {
    kind: DescriptorKind,
    name: String,
    origin: Origin,
    header: Option<DescriptorHeader>,
    is_abstract: bool,
    relationship_flavor: Option<RelationshipFlavor>,
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
    literal_properties: map_schema_semantic::LiteralObject,
    instance_properties: Vec<String>,
    instance_relationships: Vec<String>,
    literal_relationships: Vec<LiteralRelationship>,
}

pub fn compile_inputs(inputs: &[PathBuf], out_dir: &Path) -> Result<Vec<PathBuf>> {
    let lowered = lower_inputs_to_schema_ir(inputs)?;
    let compilation = build_compilation(lowered)?;
    if !compilation.diagnostics.is_empty() {
        return Err(anyhow!(format_diagnostics(&compilation.diagnostics)));
    }

    let mut written = Vec::new();
    for file in &compilation.files {
        let output = out_dir.join(file.relative_path.with_extension("json"));
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }

        let contents = emit_loader_document_json(&file.document)?;
        fs::write(&output, contents)
            .with_context(|| format!("writing compiled JSON to {}", output.display()))?;
        written.push(output);
    }

    Ok(written)
}

pub fn check_inputs(inputs: &[PathBuf]) -> Result<Vec<Diagnostic>> {
    Ok(lower_inputs_to_schema_ir(inputs)?.diagnostics)
}

struct Compilation {
    files: Vec<CompiledLoaderFile>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
struct CompiledLoaderFile {
    relative_path: PathBuf,
    document: LoaderDocument,
}

#[derive(Debug, Clone)]
struct LoweredTdlFile {
    parsed: ParsedTdlFile,
    schema_model: SemanticModel,
}

#[derive(Debug, Clone)]
struct LoweredTdlProject {
    files: Vec<LoweredTdlFile>,
    global_model: SemanticModel,
    symbols: SymbolIndex,
    diagnostics: Vec<Diagnostic>,
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
        let origin = Origin {
            source_kind: SourceKind::TdlSource,
            file_path: Some(file_path.clone()),
            line: None,
            column: None,
        };
        let mut schema: Option<TdlSchema> = None;
        let mut descriptors = Vec::new();

        while self.skip_blank_lines() {
            let line = self.peek_trimmed().unwrap().to_string();
            if line.starts_with("schema ") {
                if schema.is_some() {
                    return Err(anyhow!("multiple schema declarations in {}", file_path.display()));
                }
                schema = Some(self.parse_schema_decl(origin.clone())?);
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
        Ok(ParsedTdlFile { relative_path: file_path, schema, descriptors })
    }

    fn parse_schema_decl(&mut self, origin: Origin) -> Result<TdlSchema> {
        let line = self.consume_trimmed().unwrap();
        let header = parse_inline_header(&line, "schema")?;
        let name = header.name;
        let mut dependencies = Vec::new();
        let mut literal_properties = map_schema_semantic::LiteralObject::new();
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
                    dependencies.push(current["depends_on ".len()..].trim().to_string());
                    self.consume_trimmed();
                } else if current == "properties {" {
                    self.consume_trimmed();
                    let (properties, _instance_properties) = self.parse_properties_block()?;
                    literal_properties.extend(
                        properties.iter().map(|(key, value)| (key.clone(), value.clone())),
                    );
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
            origin,
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
        let mut descriptor = TdlDescriptor {
            kind: parsed.kind,
            name: parsed.name,
            origin: Origin {
                source_kind: SourceKind::TdlSource,
                file_path: Some(self.relative_path.clone()),
                line: Some(self.current_line_number().saturating_sub(1) as u32),
                column: Some(1),
            },
            header: None,
            is_abstract: parsed.is_abstract,
            relationship_flavor: parsed.relationship_flavor,
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
            literal_properties: map_schema_semantic::LiteralObject::new(),
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
                        descriptor.header = Some(self.parse_header_block()?);
                    }
                    s if s.starts_with("extends ") => {
                        descriptor.extends = Some(s["extends ".len()..].trim().to_string());
                        self.consume_trimmed();
                    }
                    s if s.starts_with("value ") => {
                        descriptor.value_type = Some(s["value ".len()..].trim().to_string());
                        self.consume_trimmed();
                    }
                    s if s.starts_with("source ") => {
                        descriptor.source_type = Some(s["source ".len()..].trim().to_string());
                        self.consume_trimmed();
                    }
                    s if s.starts_with("target ") => {
                        descriptor.target_type = Some(s["target ".len()..].trim().to_string());
                        self.consume_trimmed();
                    }
                    s if s.starts_with("inverse ") => {
                        let inverse_name = s["inverse ".len()..].trim().to_string();
                        if descriptor.relationship_flavor == Some(RelationshipFlavor::Inverse) {
                            descriptor.inverse_of = Some(inverse_name);
                        } else {
                            descriptor.has_inverse = Some(inverse_name);
                        }
                        self.consume_trimmed();
                    }
                    s if s.starts_with("keyrule ") => {
                        descriptor.key_rule = Some(s["keyrule ".len()..].trim().to_string());
                        self.consume_trimmed();
                    }
                    s if s.starts_with("cardinality ") => {
                        let range = s["cardinality ".len()..].trim();
                        let (min, max) = range
                            .split_once("..")
                            .ok_or_else(|| anyhow!("invalid cardinality '{}'", range))?;
                        descriptor.min_cardinality = Some(min.trim().parse()?);
                        descriptor.max_cardinality = Some(max.trim().parse()?);
                        self.consume_trimmed();
                    }
                    "ordered" => {
                        descriptor.is_ordered = true;
                        self.consume_trimmed();
                    }
                    "duplicates" => {
                        descriptor.allows_duplicates = true;
                        self.consume_trimmed();
                    }
                    "allows_additional_properties" => {
                        descriptor.allows_additional_properties = true;
                        self.consume_trimmed();
                    }
                    "allows_additional_relationships" => {
                        descriptor.allows_additional_relationships = true;
                        self.consume_trimmed();
                    }
                    s if s.starts_with("deletion_semantic ") => {
                        descriptor.deletion_semantic =
                            Some(s["deletion_semantic ".len()..].trim().to_string());
                        self.consume_trimmed();
                    }
                    "properties {" => {
                        self.consume_trimmed();
                    let (literal_properties, instance_properties) = self.parse_properties_block()?;
                    descriptor.literal_properties.extend(
                        literal_properties.iter().map(|(key, value)| (key.clone(), value.clone())),
                    );
                        descriptor.instance_properties.extend(instance_properties);
                    }
                    "relationships {" => {
                        self.consume_trimmed();
                        for line in self.parse_reference_block()? {
                            if let Some(relationship) = parse_literal_relationship_line(&line)? {
                                descriptor.literal_relationships.push(relationship);
                            } else {
                                descriptor.instance_relationships.push(line);
                            }
                        }
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
                            return Err(anyhow!("unexpected descriptor clause: {}", other));
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
        let mut descriptor = TdlDescriptor {
            kind: DescriptorKind::EnumVariant,
            name: parsed.name,
            origin: Origin {
                source_kind: SourceKind::TdlSource,
                file_path: Some(self.relative_path.clone()),
                line: Some(self.current_line_number().saturating_sub(1) as u32),
                column: Some(1),
            },
            header: None,
            is_abstract: parsed.is_abstract,
            relationship_flavor: parsed.relationship_flavor,
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
            literal_properties: map_schema_semantic::LiteralObject::new(),
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
                    descriptor.header = Some(self.parse_header_block()?);
                } else if current == "properties {" {
                    self.consume_trimmed();
                    let (literal_properties, instance_properties) = self.parse_properties_block()?;
                    descriptor.literal_properties.extend(
                        literal_properties.iter().map(|(key, value)| (key.clone(), value.clone())),
                    );
                    descriptor.instance_properties.extend(instance_properties);
                } else if current == "relationships {" {
                    self.consume_trimmed();
                    for line in self.parse_reference_block()? {
                        if let Some(relationship) = parse_literal_relationship_line(&line)? {
                            descriptor.literal_relationships.push(relationship);
                        } else {
                            descriptor.instance_relationships.push(line);
                        }
                    }
                } else if current.starts_with("extends ") {
                    descriptor.extends = Some(current["extends ".len()..].trim().to_string());
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

    fn parse_properties_block(
        &mut self,
    ) -> Result<(map_schema_semantic::LiteralObject, Vec<String>)> {
        let mut properties = map_schema_semantic::LiteralObject::new();
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

    fn current_line_number(&self) -> usize {
        self.index + 1
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
    Ok(InlineHeader { name: remainder.to_string(), header: None, has_block })
}

struct InlineHeader {
    name: String,
    header: Option<DescriptorHeader>,
    has_block: bool,
}

fn parse_descriptor_header(line: &str) -> Result<ParsedHead> {
    let trimmed = line.trim();
    let has_block = trimmed.ends_with('{');
    let head = if has_block { trimmed.trim_end_matches('{').trim() } else { trimmed };
    let mut is_abstract = false;
    let mut is_definitional = false;
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
        let (kind, tail) = if let Some(tail) = remainder.strip_prefix("schema ") {
            (DescriptorKind::Schema, tail)
        } else if let Some(tail) = remainder.strip_prefix("holon ") {
            (DescriptorKind::HolonType, tail)
        } else if let Some(tail) = remainder.strip_prefix("value ") {
            (DescriptorKind::ValueType, tail)
        } else if let Some(tail) = remainder.strip_prefix("enum ") {
            (DescriptorKind::Enum, tail)
        } else if let Some(tail) = remainder.strip_prefix("property ") {
            (DescriptorKind::PropertyType, tail)
        } else if let Some(tail) = remainder.strip_prefix("relationship ") {
            (DescriptorKind::RelationshipType, tail)
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
    let name = after_kind.trim_end_matches('{').trim().to_string();
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
        || line.starts_with("variant ")
}

fn parse_string_literal(raw: &str) -> Result<String> {
    if raw.starts_with('"') {
        Ok(serde_json::from_str(raw)?)
    } else {
        Ok(raw.to_string())
    }
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

fn parse_literal_property_line(
    line: &str,
) -> Result<Option<(String, map_schema_semantic::LiteralValue)>> {
    let Some((name, raw_value)) = line.split_once(':') else {
        return Ok(None);
    };

    let name = name.trim();
    let raw_value = raw_value.trim();
    if name.is_empty() || raw_value.is_empty() {
        return Ok(None);
    }

    Ok(Some((name.to_string(), json_value_to_literal(&serde_json::from_str(raw_value)?))))
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
    for relationship in &descriptor.literal_relationships {
        match relationship.name.as_str() {
            "Extends" if descriptor.extends.is_none() => {
                descriptor.extends = relationship.targets.first().cloned();
            }
            "UsesKeyRule" if descriptor.key_rule.is_none() => {
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

fn lower_inputs_to_schema_ir(inputs: &[PathBuf]) -> Result<LoweredTdlProject> {
    lower_parsed_files_to_schema_ir(parse_inputs(inputs)?)
}

fn lower_parsed_files_to_schema_ir(parsed: Vec<ParsedTdlFile>) -> Result<LoweredTdlProject> {
    let mut files = Vec::new();
    let mut global_model = SemanticModel::new();

    for parsed_file in parsed {
        let file_model = lower_file_to_schema_ir(&parsed_file)?;
        let mut merge_model = file_model.clone();
        for schema in merge_model.schemas.drain(..) {
            merge_schema(&mut global_model, schema);
        }
        global_model.descriptors.extend(merge_model.descriptors);
        files.push(LoweredTdlFile { parsed: parsed_file, schema_model: file_model });
    }

    let (symbols, mut diagnostics) = SymbolIndex::build(&mut global_model);
    for file in &mut files {
        file.schema_model.resolve_references(&symbols);
        diagnostics.extend(symbols.collect_reference_diagnostics(&file.schema_model));
    }

    Ok(LoweredTdlProject { files, global_model, symbols, diagnostics })
}

fn build_compilation(lowered: LoweredTdlProject) -> Result<Compilation> {
    let LoweredTdlProject { files, global_model: _global_model, symbols, diagnostics } = lowered;
    let schema_models = files.iter().map(|file| &file.schema_model).collect::<Vec<_>>();
    let emitted_key_lookup = build_emitted_key_lookup(&schema_models);
    let mut compiled_files = Vec::with_capacity(files.len());

    for file in files {
        let mut load_with =
            collect_file_dependencies(&file.schema_model, &symbols, &file.parsed.relative_path);
        load_with.sort();
        let document = lower_schema_model_to_loader_ir(
            &file.schema_model,
            LoaderMeta {
                generator: Some(GENERATOR_NAME.to_string()),
                generated_at: Some(current_timestamp_rfc3339()?),
                export_mode: Some("by-file".to_string()),
                source_files: vec![
                    file.parsed.relative_path.with_extension("tdl").to_string_lossy().to_string(),
                ],
                load_with,
            },
            &emitted_key_lookup,
        );
        compiled_files.push(CompiledLoaderFile {
            relative_path: file.parsed.relative_path.clone(),
            document,
        });
    }

    Ok(Compilation { files: compiled_files, diagnostics })
}

fn current_timestamp_rfc3339() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

fn lower_file_to_schema_ir(file: &ParsedTdlFile) -> Result<SemanticModel> {
    let mut model = SemanticModel::new();
    model.push_schema(Schema {
        name: file.schema.name.clone(),
        key: file.schema.name.clone(),
        origin: file.schema.origin.clone(),
        dependencies: file
            .schema
            .dependencies
            .iter()
            .map(|dependency| {
                SemanticReference::unresolved(ReferenceRole::DependsOn, dependency.clone())
            })
            .collect(),
        literal_properties: file.schema.literal_properties.clone(),
        literal_relationships: file.schema.literal_relationships.clone(),
        header: file.schema.header.clone(),
        allows_additional_properties: file.schema.allows_additional_properties,
        allows_additional_relationships: file.schema.allows_additional_relationships,
    });

    for descriptor in &file.descriptors {
        model.push_descriptor(lower_descriptor(descriptor, &file.schema.name)?);
    }
    Ok(model)
}

fn lower_descriptor(descriptor: &TdlDescriptor, schema_name: &str) -> Result<TypeDescriptor> {
    let mut lowered = TypeDescriptor::new(
        descriptor_key(descriptor, schema_name),
        descriptor.name.clone(),
        descriptor.kind,
        schema_name,
        descriptor.origin.clone(),
    );
    lowered.header = descriptor.header.clone();
    lowered.is_abstract = descriptor.is_abstract;
    lowered.literal_properties = descriptor.literal_properties.clone();
    lowered.literal_relationships = descriptor.literal_relationships.clone();
    lowered.is_definitional = descriptor.is_definitional;
    lowered.min_cardinality = descriptor.min_cardinality;
    lowered.max_cardinality = descriptor.max_cardinality;
    lowered.deletion_semantic = descriptor.deletion_semantic.clone();
    lowered.is_ordered = descriptor.is_ordered;
    lowered.allows_duplicates = descriptor.allows_duplicates;
    lowered.allows_additional_properties = descriptor.allows_additional_properties;
    lowered.allows_additional_relationships = descriptor.allows_additional_relationships;
    lowered.component_of =
        Some(SemanticReference::unresolved(ReferenceRole::ComponentOf, schema_name.to_string()));

    if let Some(extends) = resolved_extends(descriptor) {
        lowered.extends = Some(SemanticReference::unresolved(ReferenceRole::Extends, extends));
    }

    if let Some(value_type) = &descriptor.value_type {
        lowered.value_type =
            Some(SemanticReference::unresolved(ReferenceRole::ValueType, value_type.clone()));
    }
    if let Some(source_type) = &descriptor.source_type {
        lowered.source_type =
            Some(SemanticReference::unresolved(ReferenceRole::SourceType, source_type.clone()));
    }
    if let Some(target_type) = &descriptor.target_type {
        lowered.target_type =
            Some(SemanticReference::unresolved(ReferenceRole::TargetType, target_type.clone()));
    }
    if let Some(inverse_of) = &descriptor.inverse_of {
        lowered.inverse_of =
            Some(SemanticReference::unresolved(ReferenceRole::InverseOf, inverse_of.clone()));
    }
    if let Some(has_inverse) = &descriptor.has_inverse {
        lowered.has_inverse =
            Some(SemanticReference::unresolved(ReferenceRole::HasInverse, has_inverse.clone()));
    }
    if let Some(key_rule) = &descriptor.key_rule {
        lowered.key_rule =
            Some(SemanticReference::unresolved(ReferenceRole::KeyRule, key_rule.clone()));
    }
    if let Some(parent) = &descriptor.variant_of {
        lowered.variant_of =
            Some(SemanticReference::unresolved(ReferenceRole::VariantOf, parent.clone()));
    }
    for variant in &descriptor.variants {
        lowered
            .variants
            .push(SemanticReference::unresolved(ReferenceRole::Variants, variant.clone()));
    }

    for target in &descriptor.instance_properties {
        lowered
            .instance_properties
            .push(SemanticReference::unresolved(ReferenceRole::InstanceProperty, target.clone()));
    }
    for target in &descriptor.instance_relationships {
        lowered.instance_relationships.push(SemanticReference::unresolved(
            ReferenceRole::InstanceRelationship,
            target.clone(),
        ));
    }

    for relationship in &descriptor.literal_relationships {
        if let Some(role) = reference_role_for_relationship_name(&relationship.name) {
            for target in &relationship.targets {
                push_reference_if_missing(
                    &mut lowered,
                    SemanticReference::unresolved(role, target.clone()),
                );
            }
        }
    }

    if descriptor.kind == DescriptorKind::RelationshipType {
        lowered.relationship_flavor = descriptor.relationship_flavor.or_else(|| {
            Some(if descriptor.inverse_of.is_some() {
                RelationshipFlavor::Inverse
            } else {
                RelationshipFlavor::Declared
            })
        });
        if lowered.extends.is_none() {
            lowered.extends = Some(SemanticReference::unresolved(
                ReferenceRole::Extends,
                if lowered.relationship_flavor == Some(RelationshipFlavor::Inverse) {
                    DEFAULT_INVERSE_RELATIONSHIP_EXTENDS.to_string()
                } else {
                    DEFAULT_DECLARED_RELATIONSHIP_EXTENDS.to_string()
                },
            ));
        }
    }

    if descriptor.kind == DescriptorKind::EnumVariant {
        if lowered.extends.is_none() {
            lowered.extends = Some(SemanticReference::unresolved(
                ReferenceRole::Extends,
                if descriptor.variant_of.is_some() {
                    DEFAULT_ENUM_VARIANT_EXTENDS.to_string()
                } else {
                    DEFAULT_VARIANT_EXTENDS.to_string()
                },
            ));
        }
    }

    if descriptor.kind == DescriptorKind::ValueType {
        if lowered.extends.is_none() {
            lowered.extends = Some(SemanticReference::unresolved(
                ReferenceRole::Extends,
                DEFAULT_VALUE_EXTENDS.to_string(),
            ));
        }
    }

    if descriptor.kind == DescriptorKind::Enum {
        if lowered.extends.is_none() {
            lowered.extends = Some(SemanticReference::unresolved(
                ReferenceRole::Extends,
                DEFAULT_ENUM_EXTENDS.to_string(),
            ));
        }
    }

    if descriptor.kind == DescriptorKind::PropertyType {
        if lowered.extends.is_none() {
            lowered.extends = Some(SemanticReference::unresolved(
                ReferenceRole::Extends,
                DEFAULT_PROPERTY_EXTENDS.to_string(),
            ));
        }
    }

    Ok(lowered)
}

fn reference_role_for_relationship_name(name: &str) -> Option<ReferenceRole> {
    match name {
        "ComponentOf" => Some(ReferenceRole::ComponentOf),
        "Extends" => Some(ReferenceRole::Extends),
        "UsesKeyRule" => Some(ReferenceRole::KeyRule),
        "SourceType" => Some(ReferenceRole::SourceType),
        "TargetType" => Some(ReferenceRole::TargetType),
        "InverseOf" => Some(ReferenceRole::InverseOf),
        "HasInverse" => Some(ReferenceRole::HasInverse),
        "ValueType" => Some(ReferenceRole::ValueType),
        "Variants" => Some(ReferenceRole::Variants),
        "VariantOf" => Some(ReferenceRole::VariantOf),
        "InstanceProperties" => Some(ReferenceRole::InstanceProperty),
        "InstanceRelationships" => Some(ReferenceRole::InstanceRelationship),
        _ => None,
    }
}

fn push_reference_if_missing(descriptor: &mut TypeDescriptor, reference: SemanticReference) {
    let already_present = descriptor
        .references()
        .into_iter()
        .any(|existing| existing.role == reference.role && existing.target == reference.target);
    if !already_present {
        crate::schema_ir::push_reference(descriptor, reference);
    }
}

fn default_extends(descriptor: &TdlDescriptor) -> Option<String> {
    match descriptor.kind {
        DescriptorKind::RelationshipType => Some(
            if descriptor.relationship_flavor == Some(RelationshipFlavor::Inverse) {
                DEFAULT_INVERSE_RELATIONSHIP_EXTENDS.to_string()
            } else {
                DEFAULT_DECLARED_RELATIONSHIP_EXTENDS.to_string()
            },
        ),
        DescriptorKind::EnumVariant => Some(if descriptor.variant_of.is_some() {
            DEFAULT_ENUM_VARIANT_EXTENDS.to_string()
        } else {
            DEFAULT_VARIANT_EXTENDS.to_string()
        }),
        DescriptorKind::ValueType => Some(DEFAULT_VALUE_EXTENDS.to_string()),
        DescriptorKind::Enum => {
            Some(if descriptor.is_abstract || descriptor.name.ends_with("ValueType") {
                DEFAULT_VALUE_EXTENDS.to_string()
            } else {
                DEFAULT_ENUM_EXTENDS.to_string()
            })
        }
        DescriptorKind::PropertyType => Some(DEFAULT_PROPERTY_EXTENDS.to_string()),
        DescriptorKind::TypeDescriptor => Some("HolonType".to_string()),
        DescriptorKind::HolonType => {
            if descriptor.name == "MetaTypeDescriptor" {
                None
            } else {
                Some("HolonType".to_string())
            }
        }
        DescriptorKind::Schema => None,
    }
}

fn resolved_extends(descriptor: &TdlDescriptor) -> Option<String> {
    descriptor.extends.clone().or_else(|| default_extends(descriptor))
}

fn holon_key_for_emit(descriptor: &TdlDescriptor) -> String {
    let Some(parent) = resolved_extends(descriptor) else {
        if descriptor.is_abstract && descriptor.name.starts_with("Meta") {
            return descriptor.name.clone();
        }
        return format!("{}.HolonType", descriptor.name);
    };

    match parent.as_str() {
        "CommandType.HolonType" => format!("{}.CommandType", descriptor.name),
        "Projection.HolonType" => format!("{}.Projection", descriptor.name),
        "DanceResponseType.HolonType" => format!("{}.DanceResponseType", descriptor.name),
        "HolonError.HolonType" => format!("{}.HolonError", descriptor.name),
        "KeyRuleType" | "KeyRuleType.HolonType" => format!("{}.KeyRuleType", descriptor.name),
        "ValueConstraintType.HolonType" => format!("{}.ValueConstraintType", descriptor.name),
        "OperatorType.HolonType" if descriptor.name != "OperatorType" => descriptor.name.clone(),
        _ if descriptor.is_abstract && parent.starts_with("Meta") => descriptor.name.clone(),
        "HolonType" => format!("{}.HolonType", descriptor.name),
        _ if parent.ends_with(".KeyRuleType") => {
            format!("{}.{}", descriptor.name, parent.trim_end_matches(".KeyRuleType"))
        }
        _ if parent.ends_with(".ValueConstraintType") => {
            format!("{}.{}", descriptor.name, parent.trim_end_matches(".ValueConstraintType"))
        }
        _ => format!("{}.HolonType", descriptor.name),
    }
}

fn descriptor_key(descriptor: &TdlDescriptor, schema_name: &str) -> String {
    match descriptor.kind {
        DescriptorKind::Schema => schema_name.to_string(),
        DescriptorKind::ValueType => descriptor.name.clone(),
        DescriptorKind::Enum => descriptor.name.clone(),
        DescriptorKind::EnumVariant => descriptor
            .variant_of
            .as_ref()
            .map(|parent| variant_key(parent, &descriptor.name))
            .unwrap_or_else(|| descriptor.name.clone()),
        DescriptorKind::PropertyType => {
            if descriptor.value_type.is_some() {
                format!("{}.PropertyType", descriptor.name)
            } else {
                descriptor.name.clone()
            }
        }
        DescriptorKind::RelationshipType => relationship_key(descriptor),
        DescriptorKind::HolonType => holon_key_for_emit(descriptor),
        DescriptorKind::TypeDescriptor => "TypeDescriptor.HolonType".to_string(),
    }
}

fn relationship_key(descriptor: &TdlDescriptor) -> String {
    let source = descriptor.source_type.clone().unwrap_or_else(|| descriptor.name.clone());
    let target = descriptor.target_type.clone().unwrap_or_else(|| descriptor.name.clone());
    format!("({source})-[{}]->({target})", descriptor.name)
}

fn variant_key(enum_name: &str, variant_name: &str) -> String {
    format!("{enum_name}.{variant_name}")
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

fn collect_file_dependencies(
    model: &SemanticModel,
    symbols: &SymbolIndex,
    current_file: &Path,
) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut collect_reference = |reference: &SemanticReference| {
        let Some(symbol_id) = reference.resolved else {
            return;
        };
        let Some(symbol) = symbols.lookup_by_id(symbol_id) else {
            return;
        };
        let Some(target_file) = symbol.origin.file_path.as_ref() else {
            return;
        };
        if target_file == current_file {
            return;
        }
        let target = target_file.with_extension("json").to_string_lossy().to_string();
        if seen.insert(target.clone()) {
            dependencies.push(target);
        }
    };

    for schema in &model.schemas {
        for reference in &schema.dependencies {
            collect_reference(reference);
        }
    }

    for descriptor in &model.descriptors {
        for reference in descriptor.references() {
            collect_reference(reference);
        }
    }

    dependencies
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile_inputs;
    use serde_json::Value;
    use std::collections::HashSet;
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("schema-src")
    }

    fn generated_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("generated")
            .join("json-imports")
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

    #[test]
    fn checks_core_schema_corpus_without_diagnostics() -> Result<()> {
        let diagnostics = check_inputs(&[fixture_dir()])?;
        assert!(diagnostics.is_empty());
        Ok(())
    }

    #[test]
    fn lowers_core_schema_corpus_into_shared_schema_ir() -> Result<()> {
        let lowered = lower_inputs_to_schema_ir(&[fixture_dir()])?;

        assert!(lowered.diagnostics.is_empty());
        assert_eq!(lowered.files.len(), 11);
        assert_eq!(lowered.global_model.schemas.len(), 3);
        assert_eq!(lowered.global_model.descriptors.len(), 317);
        assert_eq!(lowered.symbols.symbols().len(), 320);

        Ok(())
    }

    #[test]
    fn lowers_core_schema_corpus_into_loader_ir_documents() -> Result<()> {
        let lowered = lower_inputs_to_schema_ir(&[fixture_dir()])?;
        let compilation = build_compilation(lowered)?;

        assert_eq!(compilation.files.len(), 11);
        assert!(compilation.diagnostics.is_empty());

        let loader_types = compilation
            .files
            .iter()
            .find(|file| {
                file.relative_path
                    == PathBuf::from("MAP Schema Types-map-core-schema-loader-types.tdl")
            })
            .expect("loader-types TDL document");
        assert!(!loader_types.document.holons.is_empty());
        assert!(loader_types
            .document
            .holons
            .iter()
            .any(|holon| holon.key == "LoaderHolon.HolonType"));
        assert_eq!(
            loader_types.document.meta.generator.as_deref(),
            Some(GENERATOR_NAME)
        );

        Ok(())
    }

    #[test]
    fn compiles_core_schema_corpus_into_generated_json() -> Result<()> {
        let out_dir = temp_out_dir();
        let files = compile_inputs(&[fixture_dir()], &out_dir)?;

        assert_eq!(files.len(), 11);
        crate::test_support::assert_json_dir_trees_eq_ignoring_meta(&generated_fixture_dir(), &out_dir);
        Ok(())
    }

    #[test]
    fn compiled_core_schema_corpus_has_no_missing_internal_refs() -> Result<()> {
        let out_dir = temp_out_dir();
        compile_inputs(&[fixture_dir()], &out_dir)?;

        let mut emitted_keys = HashSet::new();
        let mut ref_targets = Vec::new();

        for entry in fs::read_dir(&out_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let root: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            let holons =
                root.get("holons").and_then(Value::as_array).expect("compiled file holons array");
            for holon in holons {
                let key = holon
                    .get("key")
                    .and_then(Value::as_str)
                    .expect("compiled holon key")
                    .to_string();
                emitted_keys.insert(key.clone());

                if let Some(relationships) = holon.get("relationships").and_then(Value::as_array) {
                    for relationship in relationships {
                        let relationship_name = relationship
                            .get("name")
                            .and_then(Value::as_str)
                            .expect("relationship name")
                            .to_string();
                        let target = relationship.get("target").expect("relationship target");
                        match target {
                            Value::Array(values) => {
                                for value in values {
                                    let reference = value
                                        .get("$ref")
                                        .and_then(Value::as_str)
                                        .expect("$ref target")
                                        .to_string();
                                    ref_targets.push((
                                        key.clone(),
                                        relationship_name.clone(),
                                        reference,
                                    ));
                                }
                            }
                            Value::Object(_) => {
                                let reference = target
                                    .get("$ref")
                                    .and_then(Value::as_str)
                                    .expect("$ref target")
                                    .to_string();
                                ref_targets.push((
                                    key.clone(),
                                    relationship_name.clone(),
                                    reference,
                                ));
                            }
                            other => panic!("unexpected relationship target shape: {other:?}"),
                        }
                    }
                }
            }
        }

        let missing = ref_targets
            .into_iter()
            .filter(|(_, _, target)| {
                target != "MAP Core Schema-v0.0.7"
                    && target != "QueryDance.DanceType"
                    && !emitted_keys.contains(target)
            })
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "compiled corpus contains unresolved internal refs: {missing:?}"
        );
        Ok(())
    }

    #[test]
    fn ordinary_keyword_injections_remain_keyword_driven_even_for_bootstrap_like_names() -> Result<()> {
        let input_dir = write_temp_tdl(
            "bootstrap-looking-property.tdl",
            r#"schema Example Schema-v0.0.1

abstract property MetaPropertyType
"#,
        )?;

        let lowered = lower_inputs_to_schema_ir(&[input_dir])?;
        let descriptor = lowered
            .global_model
            .descriptors
            .iter()
            .find(|descriptor| descriptor.name == "MetaPropertyType")
            .expect("MetaPropertyType descriptor");

        assert_eq!(descriptor.kind, DescriptorKind::PropertyType);
        assert_eq!(
            descriptor.extends.as_ref().map(|reference| reference.target.as_str()),
            Some(DEFAULT_PROPERTY_EXTENDS)
        );

        Ok(())
    }

    #[test]
    fn decompiler_keeps_bootstrap_anchors_out_of_ordinary_keyword_surface_forms() -> Result<()> {
        let out_dir = temp_out_dir();
        decompile_inputs(
            &[PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("host")
                .join("import_files")
                .join("map-schema")
                .join("core-schema")],
            &out_dir,
        )?;

        let root = fs::read_to_string(out_dir.join("MAP Schema Types-map-core-schema-root.tdl"))?;
        let abstract_values =
            fs::read_to_string(out_dir.join("MAP Schema Types-map-core-schema-abstract-value-types.tdl"))?;

        assert!(root.contains("abstract holon MetaPropertyType {"));
        assert!(!root.contains("abstract property MetaPropertyType {"));
        assert!(abstract_values.contains("abstract holon MetaValueType {"));
        assert!(!abstract_values.contains("abstract value MetaValueType {"));

        Ok(())
    }
}
