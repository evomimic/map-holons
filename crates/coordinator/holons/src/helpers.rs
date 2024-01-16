use hdk::prelude::*;
//use std::convert::Into;
use shared_types_holon::holon_node::{HolonNode};
use crate::holon_errors::HolonError;
use crate::holon_reference::{HolonReference, LocalHolonReference};
use crate::holon_reference::HolonReference::Local;
use crate::holon_types::Holon;
use crate::relationship::RelationshipTarget;


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

/// This helper function returns a RelationshipTarget for the specified holon
/// It assumes the holon is Local
pub fn define_local_target(holon:&Holon) ->RelationshipTarget {
    // Define a RelationshipTarget for the provided Holon
    let mut local_reference = LocalHolonReference::new();
    local_reference.with_holon(holon.clone());
    let reference : HolonReference = Local(local_reference);

    let target = RelationshipTarget::One(reference);
    target
}

