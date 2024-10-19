use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::relationship::RelationshipMap;
use crate::smart_reference::SmartReference;
use shared_types_holon::{BaseValue, HolonId, PropertyMap};

use crate::holon::*;
use crate::holon_collection::CollectionState;

use hdk::prelude::*;
use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Serialize, Serializer};

// Wrapper for HolonState
struct HolonStateWrapper<'a>(&'a HolonState);

impl<'a> Serialize for HolonStateWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            HolonState::New => serializer.serialize_unit_variant("HolonState", 0, "New"),
            HolonState::Fetched => serializer.serialize_unit_variant("HolonState", 1, "Fetched"),
            HolonState::Changed => serializer.serialize_unit_variant("HolonState", 2, "Changed"),
            HolonState::Saved => serializer.serialize_unit_variant("HolonState", 3, "Saved"),
            HolonState::Abandoned => {
                serializer.serialize_unit_variant("HolonState", 4, "Abandoned")
            }
        }
    }
}

// Wrapper for ValidationState
struct ValidationStateWrapper<'a>(&'a ValidationState);

impl<'a> Serialize for ValidationStateWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            ValidationState::NoDescriptor => {
                serializer.serialize_unit_variant("ValidationState", 0, "NoDescriptor")
            }
            ValidationState::ValidationRequired => {
                serializer.serialize_unit_variant("ValidationState", 1, "ValidationRequired")
            }
            ValidationState::Validated => {
                serializer.serialize_unit_variant("ValidationState", 2, "Validated")
            }
            ValidationState::Invalid => {
                serializer.serialize_unit_variant("ValidationState", 3, "Invalid")
            }
        }
    }
}

// Wrapper for CollectionState
struct CollectionStateWrapper<'a>(&'a CollectionState);

impl<'a> Serialize for CollectionStateWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            CollectionState::Fetched => {
                serializer.serialize_unit_variant("CollectionState", 0, "Fetched")
            }
            CollectionState::Staged => {
                serializer.serialize_unit_variant("CollectionState", 1, "Staged")
            }
            CollectionState::Saved => {
                serializer.serialize_unit_variant("CollectionState", 2, "Saved")
            }
            CollectionState::Abandoned => {
                serializer.serialize_unit_variant("CollectionState", 3, "Abandoned")
            }
        }
    }
}

// Wrapper for SmartReference
struct SmartReferenceWrapper<'a>(&'a SmartReference);

impl<'a> Serialize for SmartReferenceWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SmartReference", 2)?;

        match self.0.get_id() {
            Ok(holon_id) => state.serialize_field("holon_id", &HolonIdWrapper(&holon_id))?,
            Err(_) => state.serialize_field("holon_id", &"Error fetching ID")?, // or handle the error differently
        }

        state.serialize_field("smart_property_values", &self.0.get_smart_properties())?;
        state.end()
    }
}

// Wrapper for HolonId
struct HolonIdWrapper<'a>(&'a HolonId);

impl<'a> Serialize for HolonIdWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            HolonId::Local(local_id) => {
                serializer.serialize_str(&format!("Local({})", local_id.0.to_string()))
            }
            HolonId::External(external_id) => serializer.serialize_str(&format!(
                "External(Space: {}, Local: {})",
                external_id.space_id.0.to_string(),
                external_id.local_id.0.to_string()
            )),
        }
    }
}

// Wrapper for PropertyMap
// Wrapper for PropertyMap
struct PropertyMapWrapper<'a>(&'a PropertyMap);

impl<'a> Serialize for PropertyMapWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in self.0 {
            match v {
                BaseValue::StringValue(s) => map.serialize_entry(&k.0, &s.0)?,
                BaseValue::BooleanValue(b) => map.serialize_entry(&k.0, &b.0)?,
                BaseValue::IntegerValue(i) => map.serialize_entry(&k.0, &i.0)?,
                BaseValue::EnumValue(e) => map.serialize_entry(&k.0, &e.0)?,
            }
        }
        map.end()
    }
}

// Wrapper for RelationshipMap
struct RelationshipMapWrapper<'a>(&'a RelationshipMap);

impl<'a> Serialize for RelationshipMapWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0 .0.len()))?;
        for (k, v) in &self.0 .0 {
            map.serialize_entry(&k.0.to_string(), &HolonCollectionWrapper(v))?;
        }
        map.end()
    }
}

// Wrapper for HolonCollection
struct HolonCollectionWrapper<'a>(&'a HolonCollection);

impl<'a> Serialize for HolonCollectionWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("state", &CollectionStateWrapper(&self.0.get_state()))?;
        map.serialize_entry("members", &self.0.get_members())?;
        map.end()
    }
}

// Wrapper for HolonError
struct HolonErrorWrapper<'a>(&'a HolonError);

impl<'a> Serialize for HolonErrorWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

// Wrapper for Option<Record>
struct SavedNodeWrapper<'a>(&'a Option<Record>);

impl<'a> Serialize for SavedNodeWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Some(_) => serializer.serialize_str("Some"),
            None => serializer.serialize_str("None"),
        }
    }
}

// Wrapper for Option<SmartReference>
struct SmartReferenceOptionWrapper<'a>(&'a Option<SmartReference>);

impl<'a> Serialize for SmartReferenceOptionWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Some(ref smart) => {
                let wrapper = SmartReferenceWrapper(smart);
                wrapper.serialize(serializer)
            }
            None => serializer.serialize_none(),
        }
    }
}

// Wrapper for Holon
struct SerializableHolon<'a> {
    state: HolonStateWrapper<'a>,
    validation_state: ValidationStateWrapper<'a>,
    saved_node: SavedNodeWrapper<'a>,
    predecessor: SmartReferenceOptionWrapper<'a>,
    property_map: PropertyMapWrapper<'a>,
    relationship_map: RelationshipMapWrapper<'a>,
    errors: Vec<HolonErrorWrapper<'a>>,
}

impl<'a> Serialize for SerializableHolon<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SerializableHolon", 7)?;
        state.serialize_field("state", &self.state)?;
        state.serialize_field("validation_state", &self.validation_state)?;
        state.serialize_field("saved_node", &self.saved_node)?;
        state.serialize_field("predecessor", &self.predecessor)?;
        state.serialize_field("property_map", &self.property_map)?;
        state.serialize_field("relationship_map", &self.relationship_map)?;
        state.serialize_field("errors", &self.errors)?;
        state.end()
    }
}

pub fn as_json(holon: &Holon) -> String {
    let state_wrapper = HolonStateWrapper(&holon.state);
    let validation_state_wrapper = ValidationStateWrapper(&holon.validation_state);
    let saved_node_wrapper = SavedNodeWrapper(&holon.saved_node);
    let predecessor_wrapper = SmartReferenceOptionWrapper(&holon.predecessor);
    let property_map_wrapper = PropertyMapWrapper(&holon.property_map);
    let relationship_map_wrapper = RelationshipMapWrapper(&holon.relationship_map);
    let errors_wrappers: Vec<HolonErrorWrapper> =
        holon.errors.iter().map(|e| HolonErrorWrapper(e)).collect();

    serde_json::to_string_pretty(&SerializableHolon {
        state: state_wrapper,
        validation_state: validation_state_wrapper,
        saved_node: saved_node_wrapper,
        predecessor: predecessor_wrapper,
        property_map: property_map_wrapper,
        relationship_map: relationship_map_wrapper,
        errors: errors_wrappers,
    })
    .unwrap()
}
