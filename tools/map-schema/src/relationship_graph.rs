//! Read-only LSP projection of the lowered LoaderRefRep facts. No inferred edges.
use crate::editor_service::EditorWorkspace;
use crate::source_provenance::{LoaderFactIdentity, SourceRange};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredRelationshipGraph {
    pub version: u32,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub key: String,
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub uri: String,
    pub range: SourceRange,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub name: String,
    pub target: String,
    pub occurrence: usize,
    pub location: SourceLocation,
}

impl EditorWorkspace {
    /// Expose canonical facts without interpreting relationship descriptor endpoints.
    /// `type` is the loader JSON shorthand for the DescribedBy relationship.
    pub fn relationship_graph(&self) -> AuthoredRelationshipGraph {
        let mut graph =
            AuthoredRelationshipGraph { version: 1, nodes: Vec::new(), edges: Vec::new() };
        let mut declared = BTreeSet::new();
        let mut targets = BTreeSet::new();
        for document in self.documents() {
            for holon in document.loader_facts().files().iter().flat_map(|file| file.holons()) {
                declared.insert(holon.key().to_string());
                let location = document
                    .provenance()
                    .iter()
                    .find(|p| p.holon_key == holon.key() && p.fact == LoaderFactIdentity::Holon)
                    .map(|p| SourceLocation { uri: document.uri().into(), range: p.range.clone() });
                graph.nodes.push(GraphNode { key: holon.key().into(), location });
                let relationships = std::iter::once(("DescribedBy", vec![holon.descriptor_type()]))
                    .chain(holon.relationships().iter().map(|(name, keys)| {
                        (name.as_str(), keys.iter().map(String::as_str).collect())
                    }));
                for (name, keys) in relationships {
                    for (occurrence, target) in keys.into_iter().enumerate() {
                        targets.insert(target.to_string());
                        let fact =
                            LoaderFactIdentity::Relationship { name: name.into(), occurrence };
                        let provenance = document
                            .provenance()
                            .iter()
                            .find(|p| p.holon_key == holon.key() && p.fact == fact)
                            .expect("compiler joins every lowered fact to provenance");
                        graph.edges.push(GraphEdge {
                            source: holon.key().into(),
                            name: name.into(),
                            target: target.into(),
                            occurrence,
                            location: SourceLocation {
                                uri: document.uri().into(),
                                range: provenance.range.clone(),
                            },
                        });
                    }
                }
            }
        }
        for key in targets.difference(&declared) {
            graph.nodes.push(GraphNode { key: key.clone(), location: None });
        }
        graph.nodes.sort_by(|a, b| a.key.cmp(&b.key));
        graph.edges.sort_by(|a, b| {
            (&a.source, &a.name, a.occurrence).cmp(&(&b.source, &b.name, b.occurrence))
        });
        graph
    }
}
