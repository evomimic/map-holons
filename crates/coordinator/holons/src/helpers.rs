use hdk::prelude::*;

use shared_types_holon::holon_node::HolonNode;
use shared_types_holon::{MapString, PropertyMap, PropertyName};

use crate::holon::Holon;
use crate::holon_error::HolonError;

pub fn get_holon_node_from_record(record: Record) -> Result<HolonNode, HolonError> {
    match record.entry() {
        RecordEntry::Present(entry) => HolonNode::try_from(entry.clone())
            .or(Err(HolonError::RecordConversion("HolonNode".to_string()))),
        _ => Err(HolonError::RecordConversion("Record does not have an entry".to_string())),
    }
}

pub fn get_key_from_property_map(map: &PropertyMap) -> Option<MapString> {
    let key_option = map.get(&PropertyName(MapString("key".to_string())));
    if let Some(key) = key_option {
        Some(MapString(key.into()))
    } else {
        None
    }
}
// Standalone function to summarize a vector of Holons
pub fn summarize_holons(holons: &Vec<Holon>) -> String {
    let summaries: Vec<String> = holons.iter().map(|holon| holon.summarize()).collect();
    format!("Holons: [{}]", summaries.join(", "))
}
