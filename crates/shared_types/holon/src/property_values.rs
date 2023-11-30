use std::collections::BTreeMap;
use hdi::prelude::*;
use serde::de::value::I64Deserializer;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub enum PropertyValue {
    StringValue(StringValue),
    BooleanValue(BooleanValue),
    IntegerValue(IntegerValue),
}
type BooleanValue = bool;
type StringValue = String;
type IntegerValue = i64;

pub struct PropertyValuesMap {
    properties: BTreeMap<String, PropertyValue>,
}



