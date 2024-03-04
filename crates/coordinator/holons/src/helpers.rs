use hdk::prelude::*;
use shared_types_holon::holon_node::{HolonNode};

use crate::holon_errors::HolonError;



pub fn get_holon_node_from_record(
    record: Record,
) -> Result<HolonNode,HolonError> {
    match record.entry() {
        RecordEntry::Present(entry) => {
            HolonNode::try_from(entry.clone())
                .or(
                    Err(HolonError::RecordConversion("HolonNode".to_string()))
                    )
            }
        _ => Err(HolonError::RecordConversion("Record does not have an entry".to_string())),
    }
}


