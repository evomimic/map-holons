use crate::parse::type_header::TypeHeader;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelationshipTypeEntry {
    pub header: TypeHeader,
    pub relationship_name: String,
    pub source_owns_relationship: bool,
    pub deletion_semantic: String,
    pub load_links_immediate: bool,
    pub target_collection_type: TargetCollectionType,
    pub has_inverse: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TargetCollectionType {
    pub semantic: String,
    pub holon_type: String,
}
