//! Typed names for the separately packaged Agent Spaces schema.
use crate::{ToPropertyName, ToRelationshipName};
use base_types::MapString;
use integrity_core_types::{PropertyName, RelationshipName};

#[derive(Debug, Clone, Copy)]
pub enum AgentSpaceHolonTypeName {
    PersonAgent,
    BioSocialAgent,
}

impl AgentSpaceHolonTypeName {
    pub fn as_holon_name(self) -> MapString {
        MapString(format!("{self:?}"))
    }
    pub fn descriptor_key(self, family: Self) -> MapString {
        MapString(format!("{}.{}", self.as_holon_name(), family.as_holon_name()))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AgentSpaceRelationshipTypeName {
    HasInterior,
    IsInteriorOf,
}
impl ToRelationshipName for AgentSpaceRelationshipTypeName {
    fn to_relationship_name(self) -> RelationshipName {
        RelationshipName(MapString(format!("{self:?}")))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AgentSpacePropertyTypeName {
    FirstName,
    MiddleName,
    LastName,
}
impl ToPropertyName for AgentSpacePropertyTypeName {
    fn to_property_name(self) -> PropertyName {
        PropertyName(MapString(format!("{self:?}")))
    }
}
