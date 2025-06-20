use crate::parse::type_header::TypeHeader;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnumTypeEntry {
    pub header: TypeHeader,
    pub type_name: String,
    pub variants: Vec<String>,
}
