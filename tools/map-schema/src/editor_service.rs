//! Source-only TDL editor services.
//!
//! This module is intentionally a derived tooling layer. It lowers complete
//! documents through the ordinary TDL source path, then indexes explicit loader
//! facts and their source spelling. It does not resolve keys, access the DHT,
//! construct transactions, or compute descriptor semantics.

use crate::{inspect_loader_facts_json, tdl_compiler::compile_input_string, LoaderFactProjection};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSymbol {
    pub key: String,
    pub kind: String,
    pub uri: String,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorReference {
    pub key: String,
    pub role: String,
    pub uri: String,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorDiagnostic {
    pub message: String,
    pub severity: EditorDiagnosticSeverity,
    pub code: String,
    pub range: SourceRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorDiagnosticSeverity {
    Error,
    Warning,
}

/// A single open TDL source document and its bounded source provenance.
#[derive(Debug, Clone)]
pub struct EditorDocument {
    uri: String,
    symbols: Vec<EditorSymbol>,
    references: Vec<EditorReference>,
    diagnostics: Vec<EditorDiagnostic>,
    loader_facts: Option<LoaderFactProjection>,
}

impl EditorDocument {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn symbols(&self) -> &[EditorSymbol] {
        &self.symbols
    }

    pub fn references(&self) -> &[EditorReference] {
        &self.references
    }

    pub fn diagnostics(&self) -> &[EditorDiagnostic] {
        &self.diagnostics
    }

    pub fn loader_facts(&self) -> Option<&LoaderFactProjection> {
        self.loader_facts.as_ref()
    }

    pub fn key_at(&self, position: SourcePosition) -> Option<&str> {
        self.references
            .iter()
            .find(|reference| contains(&reference.range, position))
            .map(|reference| reference.key.as_str())
            .or_else(|| {
                self.symbols
                    .iter()
                    .find(|symbol| contains(&symbol.range, position))
                    .map(|symbol| symbol.key.as_str())
            })
    }
}

/// An immutable-on-query index over the documents available to an editor.
#[derive(Debug, Default)]
pub struct EditorWorkspace {
    documents: HashMap<String, EditorDocument>,
}

/// Produces a source-only outline for an editor buffer.
///
/// Declaration spans come from source text and complete documents are checked
/// against their lowered loader facts. This does not query a saved space or run
/// descriptor semantics.
pub fn outline_source(uri: impl Into<String>, source: &str) -> Vec<EditorSymbol> {
    analyze_document(uri.into(), source).symbols
}

impl EditorWorkspace {
    pub fn upsert_document(&mut self, uri: impl Into<String>, source: &str) -> &EditorDocument {
        let uri = uri.into();
        let document = analyze_document(uri.clone(), source);
        self.documents.insert(uri.clone(), document);
        self.documents.get(&uri).expect("inserted editor document")
    }

    pub fn document(&self, uri: &str) -> Option<&EditorDocument> {
        self.documents.get(uri)
    }

    pub fn load_source_root(&mut self, root: &Path) -> std::io::Result<()> {
        self.load_source_root_recursive(root)
    }

    fn load_source_root_recursive(&mut self, path: &Path) -> std::io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.load_source_root_recursive(&path)?;
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("tdl") {
                let source = fs::read_to_string(&path)?;
                self.upsert_document(file_uri(&path), &source);
            }
        }
        Ok(())
    }

    pub fn definitions(&self, key: &str) -> Vec<&EditorSymbol> {
        let mut results = self
            .documents
            .values()
            .flat_map(EditorDocument::symbols)
            .filter(|symbol| symbol.key == key)
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            left.uri.cmp(&right.uri).then(left.range.start.line.cmp(&right.range.start.line))
        });
        results
    }

    pub fn usages(&self, key: &str) -> Vec<&EditorReference> {
        let mut results = self
            .documents
            .values()
            .flat_map(EditorDocument::references)
            .filter(|reference| reference.key == key)
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            left.uri.cmp(&right.uri).then(left.range.start.line.cmp(&right.range.start.line))
        });
        results
    }
}

fn analyze_document(uri: String, source: &str) -> EditorDocument {
    let (mut symbols, references, mut diagnostics) = scan_source(&uri, source);
    let loader_facts = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == EditorDiagnosticSeverity::Error)
    {
        None
    } else {
        match compile_input_string(source, uri.clone()) {
            Ok(json) => match inspect_loader_facts_json(&json, uri.clone()) {
                Ok(facts) => {
                    let lowered_keys = facts
                        .files()
                        .iter()
                        .flat_map(|file| file.holons())
                        .map(|holon| holon.key().to_string())
                        .collect::<HashSet<_>>();
                    symbols.retain(|symbol| lowered_keys.contains(&symbol.key));
                    Some(facts)
                }
                Err(error) => {
                    diagnostics.push(lowering_diagnostic(source, error.to_string()));
                    None
                }
            },
            Err(error) => {
                diagnostics.push(lowering_diagnostic(source, error.to_string()));
                None
            }
        }
    };
    EditorDocument { uri, symbols, references, diagnostics, loader_facts }
}

fn lowering_diagnostic(source: &str, message: String) -> EditorDiagnostic {
    let end = source
        .lines()
        .next()
        .map(|line| SourcePosition { line: 0, character: utf16_len(line) })
        .unwrap_or(SourcePosition { line: 0, character: 0 });
    EditorDiagnostic {
        message,
        severity: EditorDiagnosticSeverity::Error,
        code: "TDL_LOWERING".to_string(),
        range: SourceRange { start: SourcePosition { line: 0, character: 0 }, end },
    }
}

fn scan_source(
    uri: &str,
    source: &str,
) -> (Vec<EditorSymbol>, Vec<EditorReference>, Vec<EditorDiagnostic>) {
    let mut symbols = Vec::new();
    let mut references = Vec::new();
    let mut diagnostics = Vec::new();
    let mut relationship_targets = false;
    let mut braces = 0_i32;

    for (line_number, raw_line) in source.lines().enumerate() {
        let line = strip_comment(raw_line);
        let trimmed = line.trim();
        let indentation = line.len() - line.trim_start().len();
        braces += trimmed.chars().filter(|character| *character == '{').count() as i32;
        braces -= trimmed.chars().filter(|character| *character == '}').count() as i32;
        let line_number = line_number as u32;

        if let Some((kind, key_start)) = declaration_start(trimmed) {
            if let Some((key, start, end)) = reference_at(trimmed, key_start) {
                symbols.push(EditorSymbol {
                    key,
                    kind: kind.to_string(),
                    uri: uri.to_string(),
                    range: range_for(line_number, raw_line, indentation + start, indentation + end),
                });
            }
        }

        for (prefix, role) in [
            ("depends_on ", "schemaDependency"),
            ("type ", "describedBy"),
            ("extends ", "extends"),
            ("value ", "valueType"),
            ("source ", "relationshipSource"),
            ("target ", "relationshipTarget"),
            ("rule_of ", "ruleOf"),
            ("instance_keyrule ", "instanceKeyRule"),
        ] {
            if prefix == "value " && declaration_start(trimmed).is_some() {
                continue;
            }
            if let Some(offset) = trimmed.strip_prefix(prefix).map(|_| prefix.len()) {
                if let Some((key, start, end)) = reference_at(trimmed, offset) {
                    references.push(EditorReference {
                        key,
                        role: role.to_string(),
                        uri: uri.to_string(),
                        range: range_for(
                            line_number,
                            raw_line,
                            indentation + start,
                            indentation + end,
                        ),
                    });
                }
            }
        }

        if let Some(offset) = trimmed.find(" extends ").map(|offset| offset + " extends ".len()) {
            if let Some((key, start, end)) = reference_at(trimmed, offset) {
                references.push(EditorReference {
                    key,
                    role: "extends".to_string(),
                    uri: uri.to_string(),
                    range: range_for(line_number, raw_line, indentation + start, indentation + end),
                });
            }
        }

        if let Some((_, raw_target)) = trimmed.split_once("->") {
            let target = raw_target.trim().trim_end_matches(',').trim();
            relationship_targets =
                target == "[" || target.starts_with('[') && !target.ends_with(']');
            if target != "[" && !target.starts_with('[') {
                let offset =
                    trimmed.len() - raw_target.len() + raw_target.find(target).unwrap_or(0);
                if let Some((key, start, end)) = reference_at(trimmed, offset) {
                    references.push(EditorReference {
                        key,
                        role: "relationshipTarget".to_string(),
                        uri: uri.to_string(),
                        range: range_for(
                            line_number,
                            raw_line,
                            indentation + start,
                            indentation + end,
                        ),
                    });
                }
            }
        } else if relationship_targets && trimmed != "]" && trimmed != "]," && !trimmed.is_empty() {
            if let Some((key, start, end)) = reference_at(trimmed, 0) {
                references.push(EditorReference {
                    key,
                    role: "relationshipTarget".to_string(),
                    uri: uri.to_string(),
                    range: range_for(line_number, raw_line, indentation + start, indentation + end),
                });
            }
        } else if relationship_targets && (trimmed == "]" || trimmed == "],") {
            relationship_targets = false;
        }
    }

    if braces != 0 {
        let line = source.lines().count().saturating_sub(1) as u32;
        diagnostics.push(EditorDiagnostic {
            message: "unbalanced braced TDL block".to_string(),
            severity: EditorDiagnosticSeverity::Error,
            code: "TDL_SYNTAX".to_string(),
            range: SourceRange {
                start: SourcePosition { line, character: 0 },
                end: SourcePosition { line, character: 0 },
            },
        });
    }
    (symbols, references, diagnostics)
}

fn declaration_start(line: &str) -> Option<(&'static str, usize)> {
    let (line, abstract_prefix_len) = match line.strip_prefix("abstract ") {
        Some(remainder) => (remainder, "abstract ".len()),
        None => (line, 0),
    };
    for (prefix, kind) in [
        ("def relationship ", "relationship"),
        ("inverse relationship ", "inverseRelationship"),
        ("schema ", "schema"),
        ("holon ", "holon"),
        ("value ", "value"),
        ("enum ", "enum"),
        ("property ", "property"),
        ("relationship ", "relationship"),
        ("instance ", "instance"),
        ("variant ", "variant"),
    ] {
        if line.starts_with(prefix) {
            return Some((kind, abstract_prefix_len + prefix.len()));
        }
    }
    None
}

fn reference_at(line: &str, offset: usize) -> Option<(String, usize, usize)> {
    let remainder = line.get(offset..)?.trim_start();
    let start = line.len() - remainder.len();
    if remainder.is_empty() || remainder.starts_with('{') || remainder.starts_with('[') {
        return None;
    }
    if remainder.starts_with('"') {
        let end = remainder[1..].find('"')? + 2;
        let token = &remainder[..end];
        let key = serde_json::from_str::<String>(token).ok()?;
        return Some((key, start, start + end));
    }
    let end = remainder
        .find(|character: char| {
            character.is_whitespace() || matches!(character, '{' | '}' | '[' | ']' | ',')
        })
        .unwrap_or(remainder.len());
    let key = remainder[..end].trim_end_matches(',').to_string();
    (!key.is_empty()).then_some((key, start, start + end))
}

fn range_for(line: u32, raw_line: &str, start: usize, end: usize) -> SourceRange {
    SourceRange {
        start: SourcePosition { line, character: utf16_len(&raw_line[..start]) },
        end: SourcePosition { line, character: utf16_len(&raw_line[..end]) },
    }
}

fn utf16_len(value: &str) -> u32 {
    value.encode_utf16().count() as u32
}

fn strip_comment(line: &str) -> &str {
    line.split_once("//").map(|(prefix, _)| prefix).unwrap_or(line)
}

fn contains(range: &SourceRange, position: SourcePosition) -> bool {
    position.line == range.start.line
        && position.line == range.end.line
        && position.character >= range.start.character
        && position.character <= range.end.character
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_source_definitions_and_authored_references_without_resolution() {
        let mut workspace = EditorWorkspace::default();
        workspace.upsert_document(
            "file:///schema.tdl",
            "schema Example\n\nholon Book.HolonType {\n  type HolonType.TypeDescriptor\n  extends TypeDescriptor\n}\n\nholon Reader.HolonType {\n  type PreviouslySaved.Type\n}\n",
        );
        assert_eq!(workspace.definitions("Book.HolonType").len(), 1);
        assert_eq!(workspace.usages("HolonType.TypeDescriptor").len(), 1);
        assert!(workspace.definitions("PreviouslySaved.Type").is_empty());
        assert!(workspace.usages("PreviouslySaved.Type").len() == 1);
    }

    #[test]
    fn preserves_incomplete_document_outline_without_claiming_loader_facts() {
        let document = analyze_document(
            "file:///schema.tdl".to_string(),
            "schema Example\n\nholon Book.HolonType {\n  type HolonType.TypeDescriptor\n",
        );
        assert_eq!(document.symbols().len(), 2);
        assert!(document.loader_facts().is_none());
        assert!(document.diagnostics().iter().any(|diagnostic| diagnostic.code == "TDL_SYNTAX"));
    }

    #[test]
    fn reports_positions_including_tdl_indentation() {
        let document = analyze_document(
            "file:///schema.tdl".to_string(),
            "schema Example\n\nholon Book.HolonType {\n  type HolonType.TypeDescriptor\n}\n",
        );
        let reference = document.references().first().expect("type reference");
        assert_eq!(reference.range.start, SourcePosition { line: 3, character: 7 });
    }
}
