//! TDL source adapter and compiler pipeline.
//!
//! This module owns TDL-specific parsing and lowering, but not the meaning of the schema. Parsed
//! declarations are converted into the canonical [`SemanticModel`], where shared symbol
//! resolution, normalization, and semantic validation run. Successful models are then projected
//! into loader JSON. Keeping that boundary explicit allows other authoring and output formats to
//! share schema semantics without inheriting TDL syntax rules.

use crate::{
    diagnostics::{format_diagnostics, Diagnostic},
    literal_bridge::json_value_to_literal,
    loader_ir::{LoaderDocument, LoaderMeta},
    schema_index::SymbolIndex,
    schema_ir::{
        DescriptorHeader, DescriptorKind, LiteralRelationship, LiteralValue, Origin, ReferenceRole,
        RelationshipFlavor, Schema, SemanticModel, SemanticReference, SourceKind, TypeDescriptor,
    },
    schema_to_loader_ir::{
        build_emitted_key_lookup, emit_loader_document_json, lower_schema_model_to_loader_ir,
    },
};
use anyhow::{anyhow, Context, Result};
use map_schema_semantic::{
    effective_boolean_property_names, validate_model, CanonicalDescriptorGraph,
};
use map_schema_semantic::{DiagnosticKind, DiagnosticLayer};
use std::{
    collections::HashMap,
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
    property_required: Option<bool>,
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

/// Compiles all TDL files discovered beneath `inputs` into corresponding loader JSON files.
///
/// Compilation is all-or-nothing with respect to diagnostics: no output is written unless the
/// complete input set parses, resolves, and passes semantic validation. Relative input paths are
/// retained beneath `out_dir`, with each `.tdl` extension replaced by `.json`.
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

/// Compiles one TDL document provided as a raw string into loader JSON.
///
/// `source_name` is retained as diagnostic provenance and loader metadata; it does not need to
/// identify a file on disk. Syntax or semantic diagnostics are returned as a formatted error and
/// no JSON is emitted.
pub fn compile_input_string(raw: &str, source_name: impl Into<PathBuf>) -> Result<String> {
    let source_name = source_name.into();
    let parsed = parse_tdl_file(raw, &source_name)
        .map_err(|error| anyhow!(format_diagnostics(&[error.into_diagnostic()])))?;
    let lowered = lower_parsed_files_to_schema_ir(vec![parsed])?;
    let compilation = build_compilation(lowered)?;
    if !compilation.diagnostics.is_empty() {
        return Err(anyhow!(format_diagnostics(&compilation.diagnostics)));
    }
    let file = compilation
        .files
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no TDL document was compiled"))?;
    emit_loader_document_json(&file.document)
}

/// Checks a project of TDL inputs without emitting loader documents.
///
/// Syntax diagnostics are collected across files. Semantic checking begins only when every file
/// parses, because lowering an incomplete project would turn missing declarations into misleading
/// reference diagnostics.
pub fn check_inputs(inputs: &[PathBuf]) -> Result<Vec<Diagnostic>> {
    let (parsed, mut diagnostics) = parse_inputs_for_check(inputs)?;
    if !diagnostics.is_empty() {
        return Ok(diagnostics);
    }
    diagnostics.extend(lower_parsed_files_to_schema_ir(parsed)?.diagnostics);
    Ok(diagnostics)
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

/// Checks one in-memory TDL document without emitting loader JSON.
///
/// The supplied source name is used in diagnostic origins in the same way as a discovered relative
/// file path.
pub fn check_input_string(raw: &str, source_name: impl Into<PathBuf>) -> Result<Vec<Diagnostic>> {
    let source_name = source_name.into();
    let (parsed, mut diagnostics) = parse_input_string_for_check(raw, &source_name);
    if !diagnostics.is_empty() {
        return Ok(diagnostics);
    }
    diagnostics.extend(
        lower_parsed_files_to_schema_ir(vec![parsed.expect("parsed TDL file")])?.diagnostics,
    );
    Ok(diagnostics)
}

/// Loads TDL inputs into the canonical semantic model used by cross-format operations.
///
/// A syntax-invalid project returns an empty model alongside its syntax diagnostics. Callers must
/// not compare or emit that placeholder model; it exists so diagnostics can remain structured at
/// the adapter boundary.
pub(crate) fn load_semantic_model(inputs: &[PathBuf]) -> Result<(SemanticModel, Vec<Diagnostic>)> {
    let (parsed, diagnostics) = parse_inputs_for_check(inputs)?;
    if !diagnostics.is_empty() {
        return Ok((SemanticModel::new(), diagnostics));
    }
    let lowered = lower_parsed_files_to_schema_ir(parsed)?;
    Ok((lowered.global_model, lowered.diagnostics))
}

/// Output-ready loader documents paired with any diagnostics found before emission.
struct Compilation {
    files: Vec<CompiledLoaderFile>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
struct CompiledLoaderFile {
    relative_path: PathBuf,
    document: LoaderDocument,
}

/// One parsed file and the file-scoped semantic model used to preserve output partitioning.
#[derive(Debug, Clone)]
struct LoweredTdlFile {
    parsed: ParsedTdlFile,
    schema_model: SemanticModel,
}

/// Project-wide lowering state shared by validation and per-file emission.
///
/// `global_model` and `symbols` provide cross-file resolution, while `files` retain the original
/// boundaries needed to emit one loader document per source file.
#[derive(Debug, Clone)]
struct LoweredTdlProject {
    files: Vec<LoweredTdlFile>,
    global_model: SemanticModel,
    symbols: SymbolIndex,
    diagnostics: Vec<Diagnostic>,
}

/// Parses inputs for compilation, stopping at the first syntax-invalid file.
///
/// Compile mode cannot produce a partial output set, so syntax diagnostics are promoted into the
/// operation-level error channel here.
fn parse_inputs(inputs: &[PathBuf]) -> Result<Vec<ParsedTdlFile>> {
    let files = collect_tdl_files(inputs)?;
    let mut parsed = Vec::with_capacity(files.len());
    for discovered in files {
        let raw = fs::read_to_string(&discovered.source_path).with_context(|| {
            format!("reading TDL source file {}", discovered.source_path.display())
        })?;
        let document = parse_tdl_file(&raw, &discovered.relative_path)
            .map_err(|error| anyhow!(format_diagnostics(&[error.into_diagnostic()])))?;
        parsed.push(document);
    }
    Ok(parsed)
}

/// Parses every discovered file and accumulates syntax diagnostics for authoring feedback.
///
/// Successfully parsed files are returned only to support the clean-input path. Callers must not
/// lower them when `diagnostics` is non-empty because they do not represent the complete project.
fn parse_inputs_for_check(inputs: &[PathBuf]) -> Result<(Vec<ParsedTdlFile>, Vec<Diagnostic>)> {
    let files = collect_tdl_files(inputs)?;
    let mut parsed = Vec::with_capacity(files.len());
    let mut diagnostics = Vec::new();

    for discovered in files {
        let raw = fs::read_to_string(&discovered.source_path).with_context(|| {
            format!("reading TDL source file {}", discovered.source_path.display())
        })?;
        match parse_tdl_file(&raw, &discovered.relative_path) {
            Ok(document) => parsed.push(document),
            Err(error) => diagnostics.push(error.into_diagnostic()),
        }
    }

    Ok((parsed, diagnostics))
}

fn parse_input_string_for_check(
    raw: &str,
    source_name: &Path,
) -> (Option<ParsedTdlFile>, Vec<Diagnostic>) {
    match parse_tdl_file(raw, source_name) {
        Ok(parsed) => (Some(parsed), Vec::new()),
        Err(error) => (None, vec![error.into_diagnostic()]),
    }
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

pub(crate) fn discovered_input_count(inputs: &[PathBuf]) -> Result<usize> {
    Ok(collect_tdl_files(inputs)?.len())
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

type ParseResult<T> = std::result::Result<T, TdlParseError>;

/// A syntax failure with its exact TDL source location, before conversion to shared diagnostics.
#[derive(Debug, Clone)]
struct TdlParseError {
    kind: DiagnosticKind,
    origin: Origin,
}

impl TdlParseError {
    fn into_diagnostic(self) -> Diagnostic {
        Diagnostic::error(DiagnosticLayer::Syntax, self.kind, Some(self.origin))
    }
}

fn parse_tdl_file(raw: &str, relative_path: &Path) -> ParseResult<ParsedTdlFile> {
    let mut parser = Parser::new(raw, relative_path);
    parser.parse_file()
}

/// Stateful, line-oriented parser for one TDL source document.
///
/// The parser builds a TDL-shaped representation only. Name resolution and schema semantics are
/// intentionally deferred until all project files have been lowered into the canonical IR.
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

    fn current_origin(&self) -> Origin {
        Origin {
            source_kind: SourceKind::TdlSource,
            file_path: Some(self.relative_path.clone()),
            line: Some(self.current_line_number() as u32),
            column: Some(1),
        }
    }

    fn previous_origin(&self) -> Origin {
        Origin {
            source_kind: SourceKind::TdlSource,
            file_path: Some(self.relative_path.clone()),
            line: Some(self.current_line_number().saturating_sub(1) as u32),
            column: Some(1),
        }
    }

    fn syntax_error(&self, kind: DiagnosticKind) -> TdlParseError {
        TdlParseError { kind, origin: self.current_origin() }
    }

    fn previous_syntax_error(&self, kind: DiagnosticKind) -> TdlParseError {
        TdlParseError { kind, origin: self.previous_origin() }
    }

    fn syntax_message(
        &self,
        context: impl Into<String>,
        message: impl Into<String>,
    ) -> TdlParseError {
        self.syntax_error(DiagnosticKind::InvalidSyntax {
            context: context.into(),
            message: message.into(),
        })
    }

    fn missing_syntax(
        &self,
        context: impl Into<String>,
        expected: impl Into<String>,
    ) -> TdlParseError {
        self.syntax_error(DiagnosticKind::MissingSyntaxElement {
            context: context.into(),
            expected: expected.into(),
        })
    }

    /// Parses exactly one schema declaration plus any number of descriptor declarations.
    ///
    /// Descriptor order is preserved for deterministic lowering. Nested declarations accumulated
    /// in `pending_descriptors` are promoted to the same file-level descriptor stream.
    fn parse_file(&mut self) -> ParseResult<ParsedTdlFile> {
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
                    return Err(self.syntax_message(
                        "schema",
                        format!("multiple schema declarations in {}", file_path.display()),
                    ));
                }
                schema = Some(self.parse_schema_decl(origin.clone())?);
            } else if is_descriptor_line(&line) {
                descriptors.push(self.parse_descriptor_decl(None)?);
                descriptors.append(&mut self.pending_descriptors);
            } else if line == "}" {
                return Err(self.syntax_message(
                    "top-level declaration",
                    format!("unexpected closing brace in {}", file_path.display()),
                ));
            } else {
                return Err(self.syntax_message(
                    "top-level declaration",
                    format!(
                        "unrecognized top-level declaration in {}: {}",
                        file_path.display(),
                        line
                    ),
                ));
            }
        }

        let schema = schema.ok_or_else(|| {
            self.missing_syntax("file", format!("schema declaration in {}", file_path.display()))
        })?;
        Ok(ParsedTdlFile { relative_path: file_path, schema, descriptors })
    }

    fn parse_schema_decl(&mut self, origin: Origin) -> ParseResult<TdlSchema> {
        let line = self.consume_trimmed().unwrap();
        let header = parse_inline_header(&line, "schema", self.previous_origin())?;
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
                    literal_properties
                        .extend(properties.iter().map(|(key, value)| (key.clone(), value.clone())));
                } else if current == "relationships {" {
                    self.consume_trimmed();
                    for source_line in self.parse_reference_block()? {
                        if let Some(relationship) = parse_literal_relationship_line(
                            &source_line.text,
                            source_line.origin.clone(),
                        )? {
                            literal_relationships.push(relationship);
                        } else {
                            return Err(TdlParseError {
                                kind: DiagnosticKind::InvalidSyntax {
                                    context: "schema relationships".to_string(),
                                    message: format!(
                                        "unexpected schema relationship line: {}",
                                        source_line.text
                                    ),
                                },
                                origin: source_line.origin,
                            });
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
                    return Err(self.syntax_message(
                        "schema clause",
                        format!("unexpected schema clause: {}", current),
                    ));
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

    fn parse_descriptor_decl(&mut self, variant_of: Option<String>) -> ParseResult<TdlDescriptor> {
        let line = self.consume_trimmed().unwrap();
        let parsed = parse_descriptor_header(&line, self.previous_origin())?;
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
            property_required: parsed.property_required,
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
                        let (min, max) = range.split_once("..").ok_or_else(|| {
                            self.syntax_message(
                                "cardinality",
                                format!("invalid cardinality '{}'", range),
                            )
                        })?;
                        descriptor.min_cardinality = Some(min.trim().parse().map_err(|error| {
                            self.syntax_message(
                                "cardinality",
                                format!("invalid minimum cardinality '{}': {error}", min.trim()),
                            )
                        })?);
                        descriptor.max_cardinality = Some(max.trim().parse().map_err(|error| {
                            self.syntax_message(
                                "cardinality",
                                format!("invalid maximum cardinality '{}': {error}", max.trim()),
                            )
                        })?);
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
                        for source_line in self.parse_reference_block()? {
                            if let Some(relationship) = parse_literal_relationship_line(
                                &source_line.text,
                                source_line.origin,
                            )? {
                                descriptor.literal_relationships.push(relationship);
                            } else {
                                descriptor.instance_relationships.push(source_line.text);
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
                            return Err(self.syntax_message(
                                "descriptor clause",
                                format!("unexpected descriptor clause: {}", other),
                            ));
                        }
                    }
                }
            }
        }

        apply_literal_properties_to_tdl_descriptor(&mut descriptor).map_err(|error| {
            self.previous_syntax_error(DiagnosticKind::InvalidSyntax {
                context: "descriptor literal properties".to_string(),
                message: error.to_string(),
            })
        })?;
        apply_literal_relationships_to_tdl_descriptor(&mut descriptor);
        normalize_relationship_pair_targets(&mut descriptor);
        Ok(descriptor)
    }

    fn parse_variant_decl(&mut self, variant_of: Option<String>) -> ParseResult<TdlDescriptor> {
        let line = self.consume_trimmed().unwrap();
        let parsed = parse_descriptor_header(&line, self.previous_origin())?;
        if parsed.kind != DescriptorKind::EnumVariant {
            return Err(self.previous_syntax_error(DiagnosticKind::InvalidSyntax {
                context: "variant declaration".to_string(),
                message: format!("expected variant declaration, found {}", line),
            }));
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
            property_required: parsed.property_required,
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
                    let (literal_properties, instance_properties) =
                        self.parse_properties_block()?;
                    descriptor.literal_properties.extend(
                        literal_properties.iter().map(|(key, value)| (key.clone(), value.clone())),
                    );
                    descriptor.instance_properties.extend(instance_properties);
                } else if current == "relationships {" {
                    self.consume_trimmed();
                    for source_line in self.parse_reference_block()? {
                        if let Some(relationship) =
                            parse_literal_relationship_line(&source_line.text, source_line.origin)?
                        {
                            descriptor.literal_relationships.push(relationship);
                        } else {
                            descriptor.instance_relationships.push(source_line.text);
                        }
                    }
                } else if current.starts_with("extends ") {
                    descriptor.extends = Some(current["extends ".len()..].trim().to_string());
                    self.consume_trimmed();
                } else {
                    return Err(self.syntax_message(
                        "variant clause",
                        format!("unexpected variant clause: {}", current),
                    ));
                }
            }
        }

        apply_literal_properties_to_tdl_descriptor(&mut descriptor).map_err(|error| {
            self.previous_syntax_error(DiagnosticKind::InvalidSyntax {
                context: "variant literal properties".to_string(),
                message: error.to_string(),
            })
        })?;
        apply_literal_relationships_to_tdl_descriptor(&mut descriptor);
        normalize_relationship_pair_targets(&mut descriptor);
        Ok(descriptor)
    }

    fn parse_variant_block(&mut self, enum_name: &str) -> ParseResult<Vec<TdlDescriptor>> {
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
                return Err(self.syntax_message(
                    "variants block",
                    format!("unexpected variants clause: {}", current),
                ));
            }
        }
        Ok(variants)
    }

    fn parse_header_block(&mut self) -> ParseResult<DescriptorHeader> {
        let line = self.consume_trimmed().unwrap();
        if !line.starts_with("header") {
            return Err(self.previous_syntax_error(DiagnosticKind::InvalidSyntax {
                context: "header block".to_string(),
                message: format!("expected header block, found {}", line),
            }));
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
            let (field, value) = current.split_once(':').ok_or_else(|| {
                self.syntax_message("header field", format!("invalid header field '{}'", current))
            })?;
            let value = parse_string_literal(value.trim(), self.current_origin(), "header field")?;
            match field.trim() {
                "description" => description = Some(value),
                "display_name" => display_name = Some(value),
                "display_plural" => display_name_plural = Some(value),
                "plural" => type_name_plural = Some(value),
                other => {
                    return Err(self.syntax_message(
                        "header field",
                        format!("unexpected header field '{}'", other),
                    ))
                }
            }
            self.consume_trimmed();
        }

        Ok(DescriptorHeader { description, display_name, display_name_plural, type_name_plural })
    }

    /// Captures reference-like block entries together with their authored locations.
    ///
    /// Origins must be snapshotted before consuming each line. Downstream literal parsing can then
    /// report the offending line rather than whichever line the parser cursor reached afterward.
    fn parse_reference_block(&mut self) -> ParseResult<Vec<SourceLine>> {
        let mut refs = Vec::new();
        while self.skip_blank_lines() {
            let current = self.peek_trimmed().unwrap().to_string();
            if current == "}" {
                self.consume_trimmed();
                break;
            }
            refs.push(SourceLine { text: current, origin: self.current_origin() });
            self.consume_trimmed();
        }
        Ok(refs)
    }

    fn parse_properties_block(
        &mut self,
    ) -> ParseResult<(map_schema_semantic::LiteralObject, Vec<String>)> {
        let mut properties = map_schema_semantic::LiteralObject::new();
        let mut refs = Vec::new();
        while self.skip_blank_lines() {
            let current = self.peek_trimmed().unwrap().to_string();
            if current == "}" {
                self.consume_trimmed();
                break;
            }
            if let Some((name, value)) =
                parse_literal_property_line(&current, self.current_origin())?
            {
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

    fn try_consume_open_brace(&mut self) -> ParseResult<bool> {
        if self.skip_blank_lines() && self.peek_trimmed() == Some("{") {
            self.index += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn expect_open_brace(&mut self) -> ParseResult<()> {
        if self.try_consume_open_brace()? {
            Ok(())
        } else {
            Err(self.missing_syntax("block", "{"))
        }
    }
}

/// Source text whose origin remains stable after the parser advances.
struct SourceLine {
    text: String,
    origin: Origin,
}

#[derive(Debug, Clone)]
struct ParsedHead {
    kind: DescriptorKind,
    name: String,
    is_abstract: bool,
    property_required: Option<bool>,
    is_definitional: bool,
    relationship_flavor: Option<RelationshipFlavor>,
    extends: Option<String>,
    has_block: bool,
}

fn parse_inline_header(line: &str, keyword: &str, origin: Origin) -> ParseResult<InlineHeader> {
    let body = line.trim();
    if !body.starts_with(keyword) {
        return Err(TdlParseError {
            kind: DiagnosticKind::InvalidSyntax {
                context: format!("{keyword} declaration"),
                message: format!("expected {} declaration", keyword),
            },
            origin,
        });
    }
    let mut remainder = body[keyword.len()..].trim();
    let has_block = remainder.ends_with('{');
    if has_block {
        remainder = remainder.trim_end_matches('{').trim();
    }
    if remainder.is_empty() {
        return Err(TdlParseError {
            kind: DiagnosticKind::MissingSyntaxElement {
                context: format!("{keyword} declaration"),
                expected: format!("{keyword} name"),
            },
            origin,
        });
    }
    Ok(InlineHeader { name: remainder.to_string(), header: None, has_block })
}

struct InlineHeader {
    name: String,
    header: Option<DescriptorHeader>,
    has_block: bool,
}

fn parse_descriptor_header(line: &str, origin: Origin) -> ParseResult<ParsedHead> {
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
            return Err(TdlParseError {
                kind: DiagnosticKind::InvalidSyntax {
                    context: "descriptor declaration".to_string(),
                    message: format!("unrecognized TDL declaration: {}", line),
                },
                origin,
            });
        };
        (kind, tail.trim())
    };

    if after_kind.is_empty() {
        return Err(TdlParseError {
            kind: DiagnosticKind::MissingSyntaxElement {
                context: "descriptor declaration".to_string(),
                expected: format!("declaration name in '{}'", line),
            },
            origin,
        });
    }
    let authored_name = after_kind.trim_end_matches('{').trim();
    let property_required = if kind == DescriptorKind::PropertyType {
        Some(!authored_name.ends_with('?'))
    } else {
        None
    };
    let name = if property_required == Some(false) {
        authored_name.trim_end_matches('?').to_string()
    } else {
        authored_name.to_string()
    };
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
        property_required,
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

fn parse_string_literal(raw: &str, origin: Origin, context: &str) -> ParseResult<String> {
    if raw.starts_with('"') {
        serde_json::from_str(raw).map_err(|error| TdlParseError {
            kind: DiagnosticKind::InvalidSyntax {
                context: context.to_string(),
                message: format!("invalid string literal: {error}"),
            },
            origin,
        })
    } else {
        Ok(raw.to_string())
    }
}

fn parse_literal_relationship_line(
    line: &str,
    origin: Origin,
) -> ParseResult<Option<LiteralRelationship>> {
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
        serde_json::from_str::<Vec<String>>(raw_targets).map_err(|error| TdlParseError {
            kind: DiagnosticKind::InvalidSyntax {
                context: "relationship literal".to_string(),
                message: format!("invalid relationship target list: {error}"),
            },
            origin: origin.clone(),
        })?
    } else if raw_targets.starts_with('"') {
        vec![serde_json::from_str::<String>(raw_targets).map_err(|error| TdlParseError {
            kind: DiagnosticKind::InvalidSyntax {
                context: "relationship literal".to_string(),
                message: format!("invalid relationship target string: {error}"),
            },
            origin,
        })?]
    } else {
        vec![raw_targets.to_string()]
    };

    Ok(Some(LiteralRelationship { name: name.to_string(), targets }))
}

fn parse_literal_property_line(
    line: &str,
    origin: Origin,
) -> ParseResult<Option<(String, map_schema_semantic::LiteralValue)>> {
    let Some((name, raw_value)) = line.split_once(':') else {
        return Ok(None);
    };

    let name = name.trim();
    let raw_value = raw_value.trim();
    if name.is_empty() || raw_value.is_empty() {
        return Ok(None);
    }

    Ok(Some((
        name.to_string(),
        json_value_to_literal(&serde_json::from_str(raw_value).map_err(|error| TdlParseError {
            kind: DiagnosticKind::InvalidSyntax {
                context: "property literal".to_string(),
                message: format!("invalid property literal: {error}"),
            },
            origin,
        })?),
    )))
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

/// Runs the strict parse-and-lower path used by compilation.
fn lower_inputs_to_schema_ir(inputs: &[PathBuf]) -> Result<LoweredTdlProject> {
    lower_parsed_files_to_schema_ir(parse_inputs(inputs)?)
}

/// Lowers a complete parsed project and performs project-wide semantic validation.
///
/// TDL conventions are materialized into the canonical models before validation and projection.
/// Per-file models resolve through the global symbol index after validation, preserving source
/// partitioning while allowing cross-file references.
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
    let type_kind_defaults = derive_tdl_type_kind_defaults(&global_model, &symbols);
    apply_tdl_type_kind_defaults(&mut global_model, &type_kind_defaults);
    for file in &mut files {
        apply_tdl_type_kind_defaults(&mut file.schema_model, &type_kind_defaults);
    }
    let boolean_defaults = derive_tdl_boolean_defaults(&global_model, &symbols);
    apply_tdl_boolean_defaults(&mut global_model, &boolean_defaults);
    for file in &mut files {
        apply_tdl_boolean_defaults(&mut file.schema_model, &boolean_defaults);
    }
    expand_tdl_relationship_pairs(&mut global_model, &symbols);
    for file in &mut files {
        expand_tdl_relationship_pairs(&mut file.schema_model, &symbols);
        file.schema_model.resolve_references(&symbols);
    }
    diagnostics.extend(validate_model(&global_model, &symbols));

    Ok(LoweredTdlProject { files, global_model, symbols, diagnostics })
}

/// Propagates the TDL `value ... extends Parent` shorthand from explicit parent TypeKinds.
///
/// The propagation is computed from descriptor data and may span several files or inheritance
/// steps. Root value families whose syntax does not identify a concrete kind must author the
/// `instance_type_kind` property explicitly.
fn derive_tdl_type_kind_defaults(
    model: &SemanticModel,
    symbols: &SymbolIndex,
) -> HashMap<String, String> {
    let descriptors_by_key = model
        .descriptors
        .iter()
        .map(|descriptor| (descriptor.key.as_str(), descriptor))
        .collect::<HashMap<_, _>>();
    let mut effective = model
        .descriptors
        .iter()
        .filter_map(|descriptor| {
            descriptor
                .instance_type_kind
                .as_ref()
                .map(|kind| (descriptor.key.clone(), kind.clone()))
        })
        .collect::<HashMap<_, _>>();

    loop {
        let mut changed = false;
        for descriptor in &model.descriptors {
            if descriptor.kind != DescriptorKind::ValueType
                || descriptor.instance_type_kind.is_some()
                || effective.contains_key(&descriptor.key)
            {
                continue;
            }
            let Some(parent) = descriptor.extends.as_ref() else {
                continue;
            };
            let Some(parent_symbol) = parent
                .resolved
                .and_then(|symbol_id| symbols.lookup_by_id(symbol_id))
                .or_else(|| symbols.lookup_by_key(&parent.target))
            else {
                continue;
            };
            if !descriptors_by_key.contains_key(parent_symbol.key.as_str()) {
                continue;
            }
            let Some(parent_kind) = effective.get(&parent_symbol.key).cloned() else {
                continue;
            };
            effective.insert(descriptor.key.clone(), parent_kind);
            changed = true;
        }
        if !changed {
            break;
        }
    }

    effective
        .into_iter()
        .filter(|(key, _)| {
            descriptors_by_key.get(key.as_str()).is_some_and(|descriptor| {
                descriptor.kind == DescriptorKind::ValueType
                    && descriptor.instance_type_kind.is_none()
            })
        })
        .collect()
}

fn apply_tdl_type_kind_defaults(model: &mut SemanticModel, defaults: &HashMap<String, String>) {
    for descriptor in &mut model.descriptors {
        if descriptor.instance_type_kind.is_none() {
            descriptor.instance_type_kind = defaults.get(&descriptor.key).cloned();
        }
    }
}

/// Expands TDL's one-sided inverse shorthand into explicit canonical pair metadata.
fn expand_tdl_relationship_pairs(model: &mut SemanticModel, symbols: &SymbolIndex) {
    let descriptor_indexes = model
        .descriptors
        .iter()
        .enumerate()
        .map(|(index, descriptor)| (descriptor.key.clone(), index))
        .collect::<HashMap<_, _>>();

    for index in 0..model.descriptors.len() {
        if model.descriptors[index].kind != DescriptorKind::RelationshipType {
            continue;
        }

        let current_key = model.descriptors[index].key.clone();
        let current_symbol_id = symbols.lookup_by_key(&current_key).map(|symbol| symbol.id);
        if let Some(has_inverse) = model.descriptors[index].has_inverse.clone() {
            if let Some(target_symbol) = has_inverse
                .resolved
                .and_then(|symbol_id| symbols.lookup_by_id(symbol_id))
                .or_else(|| symbols.lookup_by_key(&has_inverse.target))
            {
                if let Some(target_index) = descriptor_indexes.get(&target_symbol.key).copied() {
                    if model.descriptors[target_index].inverse_of.is_none() {
                        model.descriptors[target_index].inverse_of = Some(SemanticReference {
                            role: ReferenceRole::InverseOf,
                            target: current_key.clone(),
                            resolved: current_symbol_id,
                        });
                    }
                }
            }
        }

        if let Some(inverse_of) = model.descriptors[index].inverse_of.clone() {
            if let Some(target_symbol) = inverse_of
                .resolved
                .and_then(|symbol_id| symbols.lookup_by_id(symbol_id))
                .or_else(|| symbols.lookup_by_key(&inverse_of.target))
            {
                if let Some(target_index) = descriptor_indexes.get(&target_symbol.key).copied() {
                    if model.descriptors[target_index].has_inverse.is_none() {
                        model.descriptors[target_index].has_inverse = Some(SemanticReference {
                            role: ReferenceRole::HasInverse,
                            target: current_key.clone(),
                            resolved: current_symbol_id,
                        });
                    }
                }
            }
        }
    }
}

/// Resolves TDL's omitted-Boolean convention through effective property descriptors.
///
/// The adapter decides that omission means `false`; the shared descriptor graph decides which
/// Boolean properties are actually applicable to each holon.
fn derive_tdl_boolean_defaults(
    model: &SemanticModel,
    symbols: &SymbolIndex,
) -> HashMap<String, Vec<String>> {
    let Ok(graph) = CanonicalDescriptorGraph::new(model, symbols) else {
        return HashMap::new();
    };
    let mut defaults = HashMap::new();
    for descriptor in &model.descriptors {
        let Some(node) = graph.node_by_key(&descriptor.key) else {
            continue;
        };
        let Some(holon) = graph.holon(node) else {
            continue;
        };
        let Ok(boolean_properties) = effective_boolean_property_names(&graph, node) else {
            continue;
        };
        let missing = boolean_properties
            .into_iter()
            .filter(|property| {
                !holon.properties.iter().any(|(name, _)| {
                    normalized_property_name(name) == normalized_property_name(property)
                })
            })
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            defaults.insert(descriptor.key.clone(), missing);
        }
    }
    defaults
}

fn apply_tdl_boolean_defaults(model: &mut SemanticModel, defaults: &HashMap<String, Vec<String>>) {
    for descriptor in &mut model.descriptors {
        let Some(properties) = defaults.get(&descriptor.key) else {
            continue;
        };
        for property in properties {
            descriptor
                .materialized_properties
                .insert(property.clone(), LiteralValue::Boolean(false));
        }
    }
}

fn normalized_property_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Projects validated semantic models into loader documents without reinterpreting TDL syntax.
///
/// Cross-file references determine each document's `load_with` metadata. The emitted-key lookup
/// ensures references use the same canonical keys as their projected loader holons.
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
                source_files: vec![file
                    .parsed
                    .relative_path
                    .with_extension("tdl")
                    .to_string_lossy()
                    .to_string()],
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

/// Converts one TDL-shaped parse result into representation-neutral schema IR.
fn lower_file_to_schema_ir(file: &ParsedTdlFile) -> Result<SemanticModel> {
    let mut model = SemanticModel::new();
    model.push_schema(Schema {
        name: file.schema.name.clone(),
        key: file.schema.name.clone(),
        origin: file.schema.origin.clone(),
        described_by: vec![SemanticReference::unresolved(
            ReferenceRole::DescribedBy,
            "Schema.HolonType",
        )],
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

/// Lowers TDL descriptor sugar and defaults into explicit canonical descriptor fields.
///
/// This function may encode TDL defaults, such as implicit `Extends` targets, but semantic
/// validity remains the responsibility of the shared validator operating on the resulting IR.
fn lower_descriptor(descriptor: &TdlDescriptor, schema_name: &str) -> Result<TypeDescriptor> {
    let kind =
        if descriptor.kind == DescriptorKind::HolonType && descriptor.name == "TypeDescriptor" {
            DescriptorKind::TypeDescriptor
        } else {
            descriptor.kind
        };
    let mut lowered = TypeDescriptor::new(
        descriptor_key(descriptor, schema_name),
        descriptor.name.clone(),
        kind,
        schema_name,
        descriptor.origin.clone(),
    );
    lowered.header = descriptor.header.clone();
    lowered.described_by.push(SemanticReference::unresolved(
        ReferenceRole::DescribedBy,
        "TypeDescriptor.HolonType",
    ));
    lowered.is_abstract = descriptor.is_abstract;
    lowered.instance_type_kind = descriptor
        .literal_properties
        .get("instance_type_kind")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .or_else(|| tdl_instance_type_kind(descriptor));
    lowered.property_required = descriptor.property_required;
    lowered.literal_properties = descriptor.literal_properties.clone();
    lowered.literal_relationships = descriptor.literal_relationships.clone();
    lowered.is_definitional = descriptor.is_definitional;
    if descriptor.kind == DescriptorKind::RelationshipType {
        lowered.min_cardinality = Some(descriptor.min_cardinality.unwrap_or(0));
        lowered.max_cardinality = Some(descriptor.max_cardinality.unwrap_or(32_767));
        lowered.deletion_semantic =
            Some(descriptor.deletion_semantic.clone().unwrap_or_else(|| "Allow".to_string()));
    } else {
        lowered.min_cardinality = descriptor.min_cardinality;
        lowered.max_cardinality = descriptor.max_cardinality;
        lowered.deletion_semantic = descriptor.deletion_semantic.clone();
    }
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

/// Lowers TDL declaration keywords into the explicit TypeKind value emitted by the adapter.
///
/// This is syntax desugaring, not validation: descriptor conformance subsequently validates the
/// resulting enum value through `TypeKind.PropertyType` and its resolved enum descriptor.
fn tdl_instance_type_kind(descriptor: &TdlDescriptor) -> Option<String> {
    let value = match descriptor.kind {
        DescriptorKind::TypeDescriptor | DescriptorKind::HolonType => "TypeKind.Holon",
        DescriptorKind::PropertyType => "TypeKind.Property",
        DescriptorKind::RelationshipType => "TypeKind.Relationship",
        DescriptorKind::Enum => "TypeKind.Value.Enum",
        DescriptorKind::EnumVariant => "TypeKind.EnumVariant",
        DescriptorKind::ValueType => return None,
        DescriptorKind::Schema => return None,
    };
    Some(value.to_string())
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
        DescriptorKind::RelationshipType => {
            Some(if descriptor.relationship_flavor == Some(RelationshipFlavor::Inverse) {
                DEFAULT_INVERSE_RELATIONSHIP_EXTENDS.to_string()
            } else {
                DEFAULT_DECLARED_RELATIONSHIP_EXTENDS.to_string()
            })
        }
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
        "DanceType.HolonType" => format!("{}.DanceType", descriptor.name),
        "DanceResponseType.HolonType" => format!("{}.DanceResponseType", descriptor.name),
        "QueryStepKind.HolonType" => format!("{}.QueryStepKind", descriptor.name),
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
    use std::{
        env, fs,
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!("{prefix}-{nanos}-{counter}"))
    }

    fn temp_out_dir() -> PathBuf {
        unique_temp_dir("map-schema-compile")
    }

    fn temp_tdl_dir() -> PathBuf {
        unique_temp_dir("map-schema-tdl")
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

    #[test]
    fn checks_core_schema_corpus_without_diagnostics() -> Result<()> {
        let diagnostics = check_inputs(&[fixture_dir()])?;
        assert!(diagnostics.is_empty(), "{}", format_diagnostics(&diagnostics));
        Ok(())
    }

    #[test]
    fn descriptor_value_conformance_accepts_core_schema_corpus() -> Result<()> {
        let lowered = lower_inputs_to_schema_ir(&[fixture_dir()])?;
        let graph = map_schema_semantic::CanonicalDescriptorGraph::new(
            &lowered.global_model,
            &lowered.symbols,
        )
        .expect("canonical descriptor graph");
        let diagnostics = map_schema_semantic::validate_canonical_model_values(&graph);
        assert!(diagnostics.is_empty(), "{} descriptor-value diagnostics", diagnostics.len());
        Ok(())
    }

    #[test]
    fn omitted_tdl_booleans_materialize_false_from_effective_descriptors() -> Result<()> {
        let lowered = lower_inputs_to_schema_ir(&[fixture_dir()])?;
        let graph = map_schema_semantic::CanonicalDescriptorGraph::new(
            &lowered.global_model,
            &lowered.symbols,
        )
        .expect("canonical descriptor graph");
        map_schema_semantic::effective_boolean_property_names(
            &graph,
            graph.node_by_key("PropertyType").expect("PropertyType node"),
        )
        .expect("PropertyType Boolean declarations should resolve");
        let expected = [
            ("PropertyType", &["IsRequired"][..]),
            (
                "Book.HolonType",
                &["AllowsAdditionalProperties", "AllowsAdditionalRelationships"][..],
            ),
            ("MetaValueType", &["AllowsAdditionalProperties", "AllowsAdditionalRelationships"][..]),
            ("DeclaredRelationshipType", &["IsDefinitional", "IsOrdered", "AllowsDuplicates"][..]),
            ("InverseRelationshipType", &["IsDefinitional", "IsOrdered", "AllowsDuplicates"][..]),
            (
                "MetaDeclaredRelationshipType",
                &["IsDefinitional", "IsOrdered", "AllowsDuplicates"][..],
            ),
            (
                "MetaInverseRelationshipType",
                &["IsDefinitional", "IsOrdered", "AllowsDuplicates"][..],
            ),
        ];

        for (key, properties) in expected {
            let descriptor = lowered
                .global_model
                .descriptors
                .iter()
                .find(|descriptor| descriptor.key == key)
                .unwrap_or_else(|| panic!("missing descriptor `{key}`"));
            for property in properties {
                assert_eq!(
                    descriptor
                        .materialized_properties
                        .get(property)
                        .and_then(LiteralValue::as_bool),
                    Some(false),
                    "`{key}.{property}` should materialize TDL omission as false"
                );
            }
        }

        Ok(())
    }

    #[test]
    fn omitted_tdl_booleans_satisfy_required_property_conformance() -> Result<()> {
        let lowered = lower_inputs_to_schema_ir(&[fixture_dir()])?;
        let graph = map_schema_semantic::CanonicalDescriptorGraph::new(
            &lowered.global_model,
            &lowered.symbols,
        )
        .expect("canonical descriptor graph");
        let diagnostics = map_schema_semantic::validate_canonical_model_conformance(&graph);
        let defaulted_boolean_properties = [
            "IsRequired",
            "IsDefinitional",
            "IsOrdered",
            "AllowsDuplicates",
            "AllowsAdditionalProperties",
            "AllowsAdditionalRelationships",
        ];
        assert!(!diagnostics.iter().any(|diagnostic| matches!(
            &diagnostic.kind,
            DiagnosticKind::MissingConformanceProperty { property, .. }
                if defaulted_boolean_properties.contains(&property.as_str())
        )));
        assert!(!diagnostics.iter().any(|diagnostic| matches!(
            &diagnostic.kind,
            DiagnosticKind::AdditionalConformanceProperty { property, .. }
                if matches!(
                    property.as_str(),
                    "AllowsAdditionalProperties" | "AllowsAdditionalRelationships"
                )
        )));
        Ok(())
    }

    #[test]
    fn invalid_type_kind_is_rejected_by_descriptor_enum_policy() -> Result<()> {
        let mut lowered = lower_inputs_to_schema_ir(&[fixture_dir()])?;
        let descriptor = lowered
            .global_model
            .descriptors
            .iter_mut()
            .find(|descriptor| descriptor.key == "Book.HolonType")
            .expect("book descriptor");
        descriptor.instance_type_kind = Some("TypeKind.HolonFoo".to_string());
        let graph = map_schema_semantic::CanonicalDescriptorGraph::new(
            &lowered.global_model,
            &lowered.symbols,
        )
        .expect("canonical descriptor graph");
        let diagnostics = map_schema_semantic::validate_canonical_holon_conformance(
            &graph,
            graph.node_by_key("Book.HolonType").expect("book node"),
        );
        assert!(
            diagnostics.iter().any(|diagnostic| matches!(
                &diagnostic.kind,
                DiagnosticKind::InvalidConformanceValue { holon, property, value, .. }
                    if holon == "Book.HolonType"
                        && property == "TypeKind"
                        && value == "TypeKind.HolonFoo"
            )),
            "{diagnostics:#?}"
        );
        Ok(())
    }

    #[test]
    fn tdl_materializes_parent_derived_value_kinds_and_inverse_pairs() -> Result<()> {
        let lowered = lower_inputs_to_schema_ir(&[fixture_dir()])?;

        for (key, expected_kind) in [
            ("MapBytesValueType", "TypeKind.Value.Bytes"),
            ("HolonIdValueType", "TypeKind.Value.Bytes"),
            ("MapValueArrayType", "TypeKind.Value.Array"),
        ] {
            let descriptor = lowered
                .global_model
                .descriptors
                .iter()
                .find(|descriptor| descriptor.key == key)
                .unwrap_or_else(|| panic!("missing descriptor `{key}`"));
            assert_eq!(descriptor.instance_type_kind.as_deref(), Some(expected_kind));
        }

        let inverse = lowered
            .global_model
            .descriptors
            .iter()
            .find(|descriptor| {
                descriptor.key == "(Schema.HolonType)-[Components]->(TypeDescriptor.HolonType)"
            })
            .expect("Components inverse descriptor");
        assert_eq!(
            inverse.inverse_of.as_ref().map(|reference| reference.target.as_str()),
            Some("(TypeDescriptor.HolonType)-[ComponentOf]->(Schema.HolonType)")
        );

        Ok(())
    }

    #[test]
    fn relationship_literal_syntax_diagnostic_uses_the_authored_line() -> Result<()> {
        let diagnostics = check_input_string(
            "schema Origin Test {\n}\n\nholon Thing {\n  relationships {\n    Broken -> [\"missing-end\"\n  }\n}\n",
            "relationship-origin.tdl",
        )?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].layer, DiagnosticLayer::Syntax);
        assert!(matches!(diagnostics[0].kind, DiagnosticKind::InvalidSyntax { .. }));
        assert_eq!(diagnostics[0].origin.as_ref().and_then(|origin| origin.line), Some(6));
        assert_eq!(diagnostics[0].origin.as_ref().and_then(|origin| origin.column), Some(1));

        Ok(())
    }

    #[test]
    fn renders_core_schema_check_output_baseline() -> Result<()> {
        let diagnostics = check_inputs(&[fixture_dir()])?;
        let rendered = render_check_output(&diagnostics);
        let expected = include_str!("../tests/baselines/core-schema-check-output.txt");
        assert_eq!(rendered, expected);
        Ok(())
    }

    #[test]
    fn lowers_core_schema_corpus_into_shared_schema_ir() -> Result<()> {
        let fixture_root = fixture_dir();
        let lowered = lower_inputs_to_schema_ir(&[fixture_root.clone()])?;

        assert!(lowered.diagnostics.is_empty());
        assert_eq!(lowered.files.len(), discovered_tdl_file_count(&fixture_root)?);
        assert!(!lowered.global_model.schemas.is_empty());
        assert!(!lowered.global_model.descriptors.is_empty());
        assert_eq!(
            lowered.symbols.symbols().len(),
            lowered.global_model.schemas.len() + lowered.global_model.descriptors.len()
        );

        Ok(())
    }

    #[test]
    fn lowers_core_schema_corpus_into_loader_ir_documents() -> Result<()> {
        let fixture_root = fixture_dir();
        let lowered = lower_inputs_to_schema_ir(&[fixture_root.clone()])?;
        let compilation = build_compilation(lowered)?;

        assert_eq!(compilation.files.len(), discovered_tdl_file_count(&fixture_root)?);
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
        assert_eq!(loader_types.document.meta.generator.as_deref(), Some(GENERATOR_NAME));

        Ok(())
    }

    #[test]
    fn compiles_core_schema_corpus_into_generated_json() -> Result<()> {
        let fixture_root = fixture_dir();
        let out_dir = temp_out_dir();
        let files = compile_inputs(&[fixture_root.clone()], &out_dir)?;

        assert_eq!(files.len(), discovered_tdl_file_count(&fixture_root)?);
        crate::test_support::assert_json_dir_trees_eq_ignoring_meta(
            &generated_fixture_dir(),
            &out_dir,
        );
        Ok(())
    }

    #[test]
    fn compiled_core_schema_corpus_has_no_missing_internal_refs() -> Result<()> {
        let out_dir = temp_out_dir();
        compile_inputs(&[fixture_dir()], &out_dir)?;
        let lowered = crate::lower_inputs_to_schema_ir(&[out_dir.clone()])?;

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
                    && lowered.symbols.lookup_reference_target(target).is_none()
            })
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "compiled corpus contains unresolved internal refs: {missing:?}"
        );
        Ok(())
    }

    #[test]
    fn ordinary_keyword_injections_remain_keyword_driven_even_for_bootstrap_like_names(
    ) -> Result<()> {
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
        let abstract_values = fs::read_to_string(
            out_dir.join("MAP Schema Types-map-core-schema-abstract-value-types.tdl"),
        )?;

        assert!(root.contains("abstract holon MetaPropertyType {"));
        assert!(!root.contains("abstract property MetaPropertyType {"));
        assert!(abstract_values.contains("abstract holon MetaValueType {"));
        assert!(!abstract_values.contains("abstract value MetaValueType {"));

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
