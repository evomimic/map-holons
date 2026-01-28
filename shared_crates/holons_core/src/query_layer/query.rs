use derive_new::new;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::reference_layer::{HolonReference, HolonsContextBehavior, ReadableHolon};
// use crate::HolonCollection;
use core_types::{HolonError, RelationshipName};

#[derive(new, Debug, Clone)]
pub struct Node {
    pub source_holon: HolonReference,
    pub relationships: Option<QueryPathMap>,
}

#[derive(Debug, Clone)]
pub struct NodeCollection {
    pub members: Vec<Node>,
    pub query_spec: Option<QueryExpression>,
}

impl NodeCollection {
    pub fn new_empty() -> Self {
        Self { members: Vec::new(), query_spec: None }
    }
}

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct QueryPathMap(pub BTreeMap<RelationshipName, NodeCollection>);

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct QueryExpression {
    pub relationship_name: RelationshipName,
}

pub fn evaluate_query(
    node_collection: NodeCollection,
    relationship_name: RelationshipName,
) -> Result<NodeCollection, HolonError> {
    let mut result_collection = NodeCollection::new_empty();

    for node in node_collection.members {
        // Fetch and lock the related holon collection
        let related_holons_lock = node.source_holon.related_holons(&relationship_name)?;
        let related_holons = related_holons_lock.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon collection: {}",
                e
            ))
        })?;

        let mut query_path_map = QueryPathMap::new(BTreeMap::new());

        for reference in related_holons.get_members() {
            let mut related_collection = NodeCollection::new_empty();
            related_collection.members.push(Node::new(reference.clone(), None));
            query_path_map.0.insert(relationship_name.clone(), related_collection);
        }

        let new_node = Node::new(node.source_holon.clone(), Some(query_path_map));
        result_collection.members.push(new_node);
    }
    Ok(result_collection)
}
