//! Immutable editor indexes derived from compiler events and canonical loader facts.

pub use crate::source_provenance::{SourcePosition, SourceRange};
use crate::{
    source_provenance::{FactProvenance, SourceEvent},
    tdl_compiler::{analyze_sources, SourceAnalysis},
    LoaderFactProjection,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

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

/// A document in an immutable compiler-backed workspace snapshot.
#[derive(Debug, Clone)]
pub struct EditorDocument {
    uri: String,
    symbols: Vec<EditorSymbol>,
    references: Vec<EditorReference>,
    diagnostics: Vec<EditorDiagnostic>,
    analysis: SourceAnalysis,
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
    pub fn loader_facts(&self) -> &LoaderFactProjection {
        &self.analysis.loader_facts
    }
    pub fn provenance(&self) -> &[FactProvenance] {
        &self.analysis.provenance
    }
    pub fn source_events(&self) -> &[SourceEvent] {
        &self.analysis.events
    }

    /// Find the key under the caret, including a selection ending immediately after its token.
    pub fn key_at(&self, position: SourcePosition) -> Option<&str> {
        let candidates = || {
            self.references
                .iter()
                .map(|reference| (&reference.range, reference.key.as_str()))
                .chain(self.symbols.iter().map(|symbol| (&symbol.range, symbol.key.as_str())))
        };
        // LSP ranges remain half-open. Navigation also accepts a caret at a token's end,
        // but an actual token under the caret takes precedence over the preceding token.
        candidates()
            .find(|(range, _)| range.contains(position))
            .or_else(|| {
                candidates().find(|(range, _)| range.start < range.end && range.end == position)
            })
            .map(|(_, key)| key)
    }
}

/// Source buffers are replaced atomically with a newly derived immutable index on each edit.
#[derive(Debug, Default)]
pub struct EditorWorkspace {
    sources: BTreeMap<String, String>,
    documents: BTreeMap<String, EditorDocument>,
    disk_paths: BTreeMap<String, PathBuf>,
}

/// Compiler-owned declaration events also provide outlines for incomplete buffers.
pub fn outline_source(uri: impl Into<String>, source: &str) -> Vec<EditorSymbol> {
    analyze_document(uri.into(), source).symbols
}

impl EditorWorkspace {
    pub fn upsert_document(&mut self, uri: impl Into<String>, source: &str) -> &EditorDocument {
        let uri = uri.into();
        self.sources.insert(uri.clone(), source.into());
        self.rebuild();
        &self.documents[&uri]
    }

    pub fn close_document(&mut self, uri: &str) {
        // Closing ends buffer ownership; the saved file may have changed since initialization.
        let saved_source = self.disk_paths.get(uri).and_then(|path| fs::read_to_string(path).ok());
        if let Some(source) = saved_source {
            self.sources.insert(uri.into(), source);
        } else {
            self.sources.remove(uri);
        }
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.documents = analyze_sources(&self.sources)
            .into_iter()
            .map(|analysis| (analysis.uri.clone(), document_from_analysis(analysis)))
            .collect();
    }

    pub fn document(&self, uri: &str) -> Option<&EditorDocument> {
        self.documents.get(uri)
    }
    pub fn documents(&self) -> impl Iterator<Item = &EditorDocument> {
        self.documents.values()
    }

    pub fn load_source_root(&mut self, root: &Path) -> std::io::Result<()> {
        self.load_source_root_recursive(root)?;
        self.rebuild();
        Ok(())
    }

    fn load_source_root_recursive(&mut self, path: &Path) -> std::io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_symlink() {
                continue;
            }
            if path.is_dir() {
                self.load_source_root_recursive(&path)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("tdl") {
                let source = fs::read_to_string(&path)?;
                let uri = file_uri(&path);
                self.disk_paths.insert(uri.clone(), path);
                self.sources.entry(uri).or_insert(source);
            }
        }
        Ok(())
    }

    pub fn definitions(&self, key: &str) -> Vec<&EditorSymbol> {
        self.documents.values().flat_map(EditorDocument::symbols).filter(|s| s.key == key).collect()
    }

    pub fn usages(&self, key: &str) -> Vec<&EditorReference> {
        self.documents
            .values()
            .flat_map(EditorDocument::references)
            .filter(|r| r.key == key)
            .collect()
    }

    pub fn symbols(&self, query: &str) -> Vec<&EditorSymbol> {
        self.documents
            .values()
            .flat_map(EditorDocument::symbols)
            .filter(|symbol| symbol.key.to_lowercase().contains(&query.to_lowercase()))
            .collect()
    }
}

fn analyze_document(uri: String, source: &str) -> EditorDocument {
    document_from_analysis(analyze_sources(&BTreeMap::from([(uri, source.into())])).remove(0))
}

fn document_from_analysis(analysis: SourceAnalysis) -> EditorDocument {
    let uri = analysis.uri.clone();
    let symbols = analysis
        .events
        .iter()
        .enumerate()
        .filter(|(index, event)| {
            *index == event.declaration && event.role.is_none() && event.key.is_some()
        })
        .map(|(_, event)| EditorSymbol {
            key: event.key.clone().unwrap(),
            kind: event.kind.clone(),
            uri: uri.clone(),
            range: event.range.clone(),
        })
        .collect();
    let references = analysis
        .events
        .iter()
        .filter(|event| matches!(event.kind.as_str(), "reference" | "relationshipName"))
        .filter_map(|event| {
            Some(EditorReference {
                key: event.key.clone()?,
                role: event.role.clone()?,
                uri: uri.clone(),
                range: event.range.clone(),
            })
        })
        .collect();
    let diagnostics = analysis
        .diagnostics
        .iter()
        .map(|d| EditorDiagnostic {
            message: d.message.clone(),
            severity: EditorDiagnosticSeverity::Error,
            code: d.code.clone(),
            range: d.range.clone(),
        })
        .collect();
    EditorDocument { uri, symbols, references, diagnostics, analysis }
}

/// Encode file paths for use as LSP document identities.
pub(crate) fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
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
        assert!(document
            .loader_facts()
            .files()
            .iter()
            .flat_map(|file| file.holons())
            .all(|holon| holon.key() != "Book.HolonType"));
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
    #[test]
    fn shared_schema_headers_are_not_duplicates_but_conflicting_holons_are() {
        let mut workspace = EditorWorkspace::default();
        workspace.upsert_document("file:///root.tdl", "schema S\nholon A {\n type T\n}\n");
        workspace.upsert_document("file:///other.tdl", "schema S\nholon B {\n type A\n}\n");
        assert!(workspace.documents().all(|d| d.diagnostics().is_empty()));
        assert_eq!(workspace.usages("A").len(), 1);
        assert_eq!(workspace.definitions("A").len(), 1);
        let graph = workspace.relationship_graph();
        assert_eq!(graph.nodes.iter().filter(|node| node.key == "S").count(), 1);
        assert_eq!(
            graph.nodes.iter().find(|node| node.key == "S").unwrap().location.as_ref().unwrap().uri,
            "file:///root.tdl"
        );
        workspace.upsert_document("file:///other.tdl", "schema S\nholon A {\n type T\n}\n");
        assert!(workspace.documents().all(|d| d
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "TDL_DUPLICATE_KEY")));
        assert_eq!(workspace.definitions("A").len(), 2);
        assert!(!workspace
            .relationship_graph()
            .nodes
            .iter()
            .any(|node| node.key == "A" && node.location.is_some()));
        workspace.close_document("file:///other.tdl");
        assert!(workspace.documents().all(|d| d.diagnostics().is_empty()));
        assert_eq!(workspace.definitions("A").len(), 1);
    }

    #[test]
    fn graph_is_lowered_occurrences_including_type_and_opaque_targets() {
        let mut workspace = EditorWorkspace::default();
        workspace.upsert_document("file:///s.tdl", "schema S\ninstance A {\n type External.Type\n relationships {\n  Links -> [B, B, C]\n }\n}\ninstance Broken {\n type T\n");
        let graph = workspace.relationship_graph();
        let edges: Vec<_> = graph.edges.iter().filter(|edge| edge.name == "Links").collect();
        assert_eq!(
            edges.iter().map(|edge| (&*edge.target, edge.occurrence)).collect::<Vec<_>>(),
            [("B", 0), ("B", 1), ("C", 2)]
        );
        assert!(edges[0].location.range.start.character < edges[1].location.range.start.character);
        assert!(graph.edges.iter().any(|edge| edge.source == "A"
            && edge.name == "DescribedBy"
            && edge.target == "External.Type"));
        assert!(graph.nodes.iter().find(|node| node.key == "B").unwrap().location.is_none());
        assert!(!graph.nodes.iter().any(|node| node.key == "Broken"));
    }
}
