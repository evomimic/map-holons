use hdi::prelude::Record;

use base_types::MapInteger;
use core_types::{HolonError, LineageId, StoredHolonNode};
use holons_core::core_shared_objects::SavedHolon;

use crate::persistence_layer::holon_storage::decode_stored_holon_node;

/// Projects a persisted holon node into the shared-objects `SavedHolon` form.
///
/// `original_id` carries the record-derived lineage root: `None` when this holon begins its own
/// lineage, `Some(root)` when it supersedes one. Lineage is a fact about the record, not a field
/// the entry body is trusted to remember.
pub fn saved_holon_from_stored(stored: StoredHolonNode) -> SavedHolon {
    SavedHolon::new(
        stored.version_metadata.version_id,
        stored.holon_node.property_map,
        stored.version_metadata.lineage_id.map(LineageId::into_local_id),
        MapInteger(1),
    )
}

/// Constructs a `SavedHolon` from a persisted record.
///
/// Retained for the path-anchored lookup in `get_holon_by_path`, which still receives a `Record`
/// from the scaffolded path externs. Id-addressed reads go through
/// `holon_storage::get_holon` instead, which never surfaces a record at all.
pub fn try_from_record(record: Record) -> Result<SavedHolon, HolonError> {
    Ok(saved_holon_from_stored(decode_stored_holon_node(&record)?))
}
