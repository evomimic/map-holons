use crate::parse::type_header::TypeHeader;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BooleanTypeEntry {
    pub type_name: String,
    pub header: TypeHeader,
}
