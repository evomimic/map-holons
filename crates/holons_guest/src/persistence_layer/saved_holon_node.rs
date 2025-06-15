use base_types::MapInteger;
use hdi::prelude::{Record, RecordEntry};
use holons_core::{
    core_shared_objects::holon::{Holon, SavedHolon},
    HolonError,
};
use integrity_core_types::{HolonNode, LocalId};

// #[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
// pub struct HolonRecord {
//     record: Record,
// }

// impl HolonRecord {
//     /// Retrieves the `LocalId` from the underlying `Record`.
//     pub fn get_local_id(&self) -> LocalId {
//             LocalId(self.record.action_address().clone())
//     }

// }
// ****

/// Constructs a `SavedHolon` from a `HolonNode`.
///
/// This method is called during deserialization from persisted records.
pub fn try_from_record(record: Record) -> Result<Holon, HolonError> {
    let holon_node = get_holon_node_from_record(record.clone())?;

    let holon = SavedHolon::new(
        LocalId(record.action_address().clone()),
        holon_node.property_map,
        holon_node.original_id,
        MapInteger(1),
    );

    Ok(Holon::Saved(holon))
}

/// Inflates a 'HolonNode' from the underlying saved 'Record'.
///
/// Private helper called by try_from_record.
fn get_holon_node_from_record(record: Record) -> Result<HolonNode, HolonError> {
    match record.entry() {
        RecordEntry::Present(entry) => HolonNode::try_from(entry.clone())
            .or(Err(HolonError::RecordConversion("HolonNode".to_string()))),
        _ => Err(HolonError::RecordConversion("Record does not have an entry".to_string())),
    }
}
