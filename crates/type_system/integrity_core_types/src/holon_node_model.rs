use crate::{LocalId, PropertyMap};
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
/// # Conversion
/// Implement `From<HolonNode>` for `HolonNodeModel` in the guest crate
/// to bridge between guest entries and validation logic.
#[derive(new, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HolonNodeModel {
    pub original_id: Option<LocalId>,
    pub property_map: PropertyMap,
}
