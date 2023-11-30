use hdi::prelude::*;
//use holon::Holon;
use crate::object_ref::HolonReference;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct Holon {
    pub descriptor: HolonReference,

}

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct HolonCollection {
    pub item_type: HolonReference, // HolonDescriptor
    pub items : Vec<HolonReference>,


}