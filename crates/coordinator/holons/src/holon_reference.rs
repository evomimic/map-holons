use crate::holon::HolonGetters;
use crate::holon_errors::HolonError;
use crate::holon_types::Holon;
use hdk::prelude::*;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::HolonId;
use shared_types_holon::value_types::BaseValue;


pub trait HolonReferenceFns {
    fn get_holon(&self) -> Result<Holon, HolonError>;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HolonReference {
    Local(LocalHolonReference),
    // External(ExternalHolonReference),
}

impl HolonReferenceFns for HolonReference {
    fn get_holon(&self) -> Result<Holon, HolonError> {
        match self {
            HolonReference::Local(holon_reference) => holon_reference.get_holon(),
        }
    }
}

impl HolonGetters for HolonReference {
    fn get_property_value(
        &self,
        property_name: PropertyName,
    ) -> Result<Option<BaseValue>, HolonError> {
        let holon = self.get_holon()?;
        holon.get_property_value(property_name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LocalHolonReference {
    holon_id: Option<HolonId>,
    holon: Option<Holon>,
}

impl HolonReferenceFns for LocalHolonReference {
    /// get_holon will return the cached Holon, first retrieving it from the storage tier, if necessary
    fn get_holon(&self) -> Result<Holon, HolonError> {
        let holon_reference = self.clone();
        if let Some(holon) = holon_reference.holon {
            Ok(holon)
        } else {
            if let Some(id) = holon_reference.holon_id {
                Holon::fetch_holon(id)
            } else {
                Err(HolonError::HolonNotFound(
                    "LocalHolonReference is empty".to_string(),
                ))
            }
        }
    }
}

impl LocalHolonReference {

    // Constructor function for creating from HolonId
    pub fn from_holon_id(holon_id: HolonId) -> Self {
        Self {
            holon_id: Some(holon_id),
            holon: None,
        }
    }

    // Constructor function for creating from Holon
    pub fn from_holon(holon: Holon) -> Self {
        Self {
            holon_id: None,
            holon: Some(holon),
        }
    }
    pub fn add_holon_id(&mut self, holon_id: HolonId)-> &mut Self {
        self.holon_id = Some(holon_id);
        self
    }
    pub fn add_holon(&mut self, holon: Holon) -> &mut Self {
        self.holon = Some(holon);
        self
    }
}
