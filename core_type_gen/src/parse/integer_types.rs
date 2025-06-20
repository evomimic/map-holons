use crate::parse::type_header::TypeHeader;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IntegerTypeEntry {
    pub header: TypeHeader,
    pub type_name: String,
    pub min_value: String, // stay as string for now due to extreme bounds
    pub max_value: String,
}
