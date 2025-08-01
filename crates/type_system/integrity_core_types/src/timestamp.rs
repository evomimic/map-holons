use serde::{Serialize, Deserialize};


#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PersistenceTimestamp(pub i64);