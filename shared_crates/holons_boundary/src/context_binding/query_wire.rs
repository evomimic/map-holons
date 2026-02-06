use crate::HolonReferenceWire;
use core_types::{HolonError, RelationshipName};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::query_layer::{Node, NodeCollection, QueryExpression, QueryPathMap};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Wire-form query node for IPC.
///
/// This is a context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime using `bind(context)`.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct QueryPathMapWire(pub BTreeMap<RelationshipName, NodeCollectionWire>);

impl NodeWire {
    pub fn new(source_holon: HolonReferenceWire, relationships: Option<QueryPathMapWire>) -> Self {
        Self { source_holon, relationships }
    }
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<Node, HolonError> {
        Ok(Node::new(
            self.source_holon.bind(context)?,
            self.relationships.map(|wire_map| wire_map.bind(context)).transpose()?,
        ))
    }
}

impl NodeCollectionWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<NodeCollection, HolonError> {
        let mut members = Vec::with_capacity(self.members.len());
        for wire_node in self.members {
            members.push(wire_node.bind(context)?);
        }

        Ok(NodeCollection { members, query_spec: self.query_spec })
    }
}

impl QueryPathMapWire {
    pub fn new(map: BTreeMap<RelationshipName, NodeCollectionWire>) -> Self {
        Self(map)
    }
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<QueryPathMap, HolonError> {
        let mut map = BTreeMap::new();
        for (relationship_name, node_collection_wire) in self.0 {
            map.insert(relationship_name, node_collection_wire.bind(context)?);
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
        QueryPathMapWire::new(
            map.0
                .iter()
                .map(|(relationship_name, node_collection)| {
                    (relationship_name.clone(), NodeCollectionWire::from(node_collection))
                })
                .collect(),
        )
    }
}
