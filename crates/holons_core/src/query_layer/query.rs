use std::{collections::BTreeMap, rc::Rc};
use serde::{Serialize, Deserialize};
use derive_new::new;

use crate::HolonCollection;
use crate::reference_layer::{ReadableHolon, HolonReference, HolonsContextBehavior};
use core_types::HolonError;
use integrity_core_types::RelationshipName;

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Node {
    pub source_holon: HolonReference,
    pub relationships: Option<QueryPathMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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
    context: &dyn HolonsContextBehavior,
    relationship_name: RelationshipName,
) -> Result<NodeCollection, HolonError> {
    let mut result_collection = NodeCollection::new_empty();

    for node in node_collection.members {
        let related_holons_rc =
            node.source_holon.get_related_holons(context, &relationship_name)?;

        let related_holons: Rc<HolonCollection> = Rc::clone(&related_holons_rc);

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
