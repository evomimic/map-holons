use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct TemporaryId(pub Uuid);

pub fn generate_temporary_id() -> TemporaryId {
    TemporaryId(Uuid::new_v4())
}