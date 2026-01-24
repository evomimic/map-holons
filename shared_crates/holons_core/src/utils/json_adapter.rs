#![allow(dead_code)]

use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Serialize, Serializer};

use base_types::BaseValue;
use core_types::{HolonError, HolonId, LocalId, PropertyMap};

use crate::core_shared_objects::holon::state::{HolonState, ValidationState};
use crate::core_shared_objects::{CollectionState, HolonCollection, StagedRelationshipMap};
use crate::reference_layer::SmartReference;

// Wrapper for HolonState
struct HolonStateWrapper<'a>(&'a HolonState);

impl<'a> Serialize for HolonStateWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            HolonState::Mutable => serializer.serialize_unit_variant("HolonState", 0, "Mutable"),
            HolonState::Immutable => {
                serializer.serialize_unit_variant("HolonState", 1, "Immutable")
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
            CollectionState::Transient => {
                serializer.serialize_unit_variant("CollectionState", 0, "Transient")
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
            HolonId::Local(local_id) => serializer.serialize_str(&format!("Local({})", local_id)),
            HolonId::External(external_id) => serializer.serialize_str(&format!(
                "External(Space: {}, Local: {})",
                external_id.space_id.0.to_string(),
                external_id.local_id
            )),
        }
    }
}

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

// Wrapper for StagedRelationshipMap
struct StagedRelationshipMapWrapper<'a>(&'a StagedRelationshipMap);

impl<'a> Serialize for StagedRelationshipMapWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let map_len = self.0.iter().count();
        let mut map = serializer.serialize_map(Some(map_len))?;
        for (k, v) in self.0.iter() {
            // Read the RwLock to get the inner HolonCollection
            let holon_collection = v.read().unwrap();
            // Use your existing HolonCollectionWrapper
            map.serialize_entry(&k.0.to_string(), &HolonCollectionWrapper(&holon_collection))?;
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

// Wrapper for ActionHash
struct SavedNodeWrapper<'a>(&'a LocalId);

impl<'a> Serialize for SavedNodeWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
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
            Some(smart) => {
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
    local_id: SavedNodeWrapper<'a>,
    property_map: PropertyMapWrapper<'a>,
    staged_relationship_map: StagedRelationshipMapWrapper<'a>,
    errors: Vec<HolonErrorWrapper<'a>>,
}

// impl<'a> Serialize for SerializableHolon<'a> {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let mut state = serializer.serialize_struct("SerializableHolon", 7)?;
//         state.serialize_field("state", &self.state)?;
//         state.serialize_field("validation_state", &self.validation_state)?;
//         state.serialize_field("record", &self.record)?;
//         state.serialize_field("property_map", &self.property_map)?;
//         state.serialize_field("staged_relationship_map", &self.staged_relationship_map)?;
//         state.serialize_field("errors", &self.errors)?;
//         state.end()
//     }
// }

// pub fn as_json(holon: &Holon) -> String {
//     let state_wrapper = HolonStateWrapper(&holon.state);
//     let validation_state_wrapper = ValidationStateWrapper(&holon.validation_state);
//     let record_wrapper = SavedNodeWrapper(&holon.record);
//     let property_map_wrapper = PropertyMapWrapper(&holon.property_map);
//     let relationship_map_wrapper = StagedRelationshipMapWrapper(&holon.staged_relationship_map);
//     let errors_wrappers: Vec<HolonErrorWrapper> =
//         holon.errors.iter().map(|e| HolonErrorWrapper(e)).collect();

//     serde_json::to_string_pretty(&SerializableHolon {
//         state: state_wrapper,
//         validation_state: validation_state_wrapper,
//         record: record_wrapper,
//         property_map: property_map_wrapper,
//         staged_relationship_map: relationship_map_wrapper,
//         errors: errors_wrappers,
//     })
//     .unwrap()
// }
