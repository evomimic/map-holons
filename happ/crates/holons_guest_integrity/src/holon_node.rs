use derive_new::new;
use hdi::prelude::*;
use integrity_core_types::{LocalId, PropertyMap};

// ===============================
// 📌 Constants
// ===============================
pub const LOCAL_HOLON_SPACE_PATH: &str = "local_holon_space";
pub const LOCAL_HOLON_SPACE_NAME: &str = "LocalHolonSpace";
pub const LOCAL_HOLON_SPACE_DESCRIPTION: &str = "Default Local Holon Space";

// ===============================
// 🌳 HolonNode Struct (holochain EntryType)
// ===============================

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub original_id: Option<LocalId>,
    pub property_map: PropertyMap,
}
