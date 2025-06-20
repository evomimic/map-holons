use crate::parse::type_header::TypeHeader;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnumVariantTypeEntry {
    pub header: TypeHeader,
    pub type_name: String,
    pub variant_order: i64,
}
