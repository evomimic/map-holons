use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::{HolonError, RelationshipName};

/// A query graph node rooted at a single holon reference.
///
/// Runtime nodes carry tx-bound `HolonReference` values and must never be
/// deserialized directly across IPC boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub source_holon: HolonReference,
    pub relationships: Option<QueryPathMap>,
}

impl Node {
    /// Creates a new runtime query node rooted at `source_holon`.
    pub fn new(source_holon: HolonReference, relationships: Option<QueryPathMap>) -> Self {
        Self { source_holon, relationships }
    }
}

/// A collection of query nodes (the query result shape).
///
/// Runtime collections carry tx-bound `HolonReference` values and must never be
/// deserialized directly across IPC boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeCollection {
    pub members: Vec<Node>,
    pub query_spec: Option<QueryExpression>,
}

impl NodeCollection {
    pub fn new_empty() -> Self {
        Self { members: Vec::new(), query_spec: None }
    }
}

/// Runtime relationship traversal map for query graphs.
///
/// This type contains tx-bound references (via `NodeCollection`) and is therefore
/// runtime-only (no `Serialize`/`Deserialize`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPathMap(pub BTreeMap<RelationshipName, NodeCollection>);

impl QueryPathMap {
    /// Creates a new relationship traversal map.
    pub fn new(map: BTreeMap<RelationshipName, NodeCollection>) -> Self {
        Self(map)
    }
}

/// A minimal query expression describing the relationship being traversed.
///
/// This does not contain tx-bound references and may be safely serialized.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct QueryExpression {
    pub relationship_name: RelationshipName,
}

impl QueryExpression {
    /// Creates a new query expression describing a traversal of `relationship_name`.
    pub fn new(relationship_name: RelationshipName) -> Self {
        Self { relationship_name }
    }
}

/// Evaluates a one-hop query by expanding each input node into related nodes.
///
/// For each `node` in `node_collection`, this fetches `relationship_name`
/// relationships and builds a `QueryPathMap` that includes all related holons
/// under that relationship key.
pub fn evaluate_query(
    node_collection: NodeCollection,
    relationship_name: RelationshipName,
) -> Result<NodeCollection, HolonError> {
    let mut result_collection = NodeCollection::new_empty();

    for node in node_collection.members {
        let related_holons_lock = node.source_holon.related_holons(&relationship_name)?;
        let related_holons = related_holons_lock.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon collection: {}",
                e
            ))
        })?;

        // Build a single related collection for this relationship (do not overwrite per member).
        let mut related_collection = NodeCollection::new_empty();
        for reference in related_holons.get_members() {
            related_collection.members.push(Node::new(reference.clone(), None));
        }

        let mut query_path_map = QueryPathMap::new(BTreeMap::new());
        query_path_map.0.insert(relationship_name.clone(), related_collection);

        result_collection.members.push(Node::new(node.source_holon.clone(), Some(query_path_map)));
    }

    Ok(result_collection)
}
