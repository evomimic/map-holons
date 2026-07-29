use crate::PropertyMap;
use derive_new::new;
use serde::{Deserialize, Serialize};

/// Holochain-independent model for a HolonNode entry.
///
/// This type is used for shared validation and application logic,
/// and intentionally avoids any dependency on Holochain types.
///
/// It is the responsibility of Holochain guest code to convert between
/// this model and the Holochain-annotated `HolonNode` struct.
///
/// # Contents
/// A holon node entry carries semantic content only. Version and lineage facts are
/// properties of the record that persists the entry, not of the entry body, and are
/// surfaced by the storage layer as `VersionMetadata`.
///
/// # Conversion
/// Implement `From<HolonNode>` for `HolonNodeModel` in the guest crate
/// to bridge between guest entries and validation logic.
#[derive(new, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HolonNodeModel {
    pub property_map: PropertyMap,
}
