use derive_new::new;
use hdi::prelude::*;
use integrity_core_types::PropertyMap;

// ===============================
// 📌 Constants
// ===============================
pub const LOCAL_HOLON_SPACE_PATH: &str = "local_holon_space";
pub const LOCAL_HOLON_SPACE_NAME: &str = "LocalHolonSpace";
pub const LOCAL_HOLON_SPACE_DESCRIPTION: &str = "Default Local Holon Space";

// ===============================
// 🌳 HolonNode Struct (holochain EntryType)
// ===============================

/// The persisted entry: semantic content only.
///
/// Version identity and lineage are facts about the record that persists this entry — a
/// `Create` begins a lineage, an `Update` extends the one it targets — so they are read from
/// the record by the storage layer rather than carried in the entry body.
#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct HolonNode {
    pub property_map: PropertyMap,
}
