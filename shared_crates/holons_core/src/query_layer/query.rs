use derive_new::new;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::{HolonReference, HolonReferenceWire, ReadableHolon};
use core_types::{HolonError, RelationshipName};

/// A query graph node rooted at a single holon reference.
///
/// Runtime nodes carry tx-bound `HolonReference` values and must never be
/// deserialized directly across IPC boundaries.
#[derive(new, Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub source_holon: HolonReference,
    pub relationships: Option<QueryPathMap>,
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
#[derive(new, Debug, Clone, PartialEq, Eq)]
pub struct QueryPathMap(pub BTreeMap<RelationshipName, NodeCollection>);

/// A minimal query expression describing the relationship being traversed.
///
/// This does not contain tx-bound references and may be safely serialized.
#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct QueryExpression {
    pub relationship_name: RelationshipName,
}

/// Wire-form query node for IPC.
///
/// This is a context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime using `bind(context)`.
#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct NodeWire {
    pub source_holon: HolonReferenceWire,
    pub relationships: Option<QueryPathMapWire>,
}

/// Wire-form node collection for IPC.
///
/// This is a context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime using `bind(context)`.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct NodeCollectionWire {
    pub members: Vec<NodeWire>,
    pub query_spec: Option<QueryExpression>,
}

/// Wire-form relationship traversal map for IPC.
///
/// This is a context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime using `bind(context)`.
#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct QueryPathMapWire(pub BTreeMap<RelationshipName, NodeCollectionWire>);

impl NodeWire {
    pub fn bind(self, context: Arc<TransactionContext>) -> Result<Node, HolonError> {
        Ok(Node::new(
            HolonReference::bind(self.source_holon, Arc::clone(&context))?,
            match self.relationships {
                None => None,
                Some(wire_map) => Some(wire_map.bind(context)?),
            },
        ))
    }
}

impl NodeCollectionWire {
    pub fn bind(self, context: Arc<TransactionContext>) -> Result<NodeCollection, HolonError> {
        let mut members = Vec::with_capacity(self.members.len());
        for wire_node in self.members {
            members.push(wire_node.bind(Arc::clone(&context))?);
        }

        Ok(NodeCollection { members, query_spec: self.query_spec })
    }
}

impl QueryPathMapWire {
    pub fn bind(self, context: Arc<TransactionContext>) -> Result<QueryPathMap, HolonError> {
        let mut map = BTreeMap::new();
        for (relationship_name, node_collection_wire) in self.0 {
            map.insert(relationship_name, node_collection_wire.bind(Arc::clone(&context))?);
        }
        Ok(QueryPathMap::new(map))
    }
}

impl From<&Node> for NodeWire {
    fn from(node: &Node) -> Self {
        Self {
            source_holon: HolonReferenceWire::from(&node.source_holon),
            relationships: node.relationships.as_ref().map(QueryPathMapWire::from),
        }
    }
}

impl From<&NodeCollection> for NodeCollectionWire {
    fn from(collection: &NodeCollection) -> Self {
        Self {
            members: collection.members.iter().map(NodeWire::from).collect(),
            query_spec: collection.query_spec.clone(),
        }
    }
}

impl From<&QueryPathMap> for QueryPathMapWire {
    fn from(map: &QueryPathMap) -> Self {
        let mut wire_map = BTreeMap::new();
        for (relationship_name, node_collection) in &map.0 {
            wire_map.insert(relationship_name.clone(), NodeCollectionWire::from(node_collection));
        }
        QueryPathMapWire::new(wire_map)
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
