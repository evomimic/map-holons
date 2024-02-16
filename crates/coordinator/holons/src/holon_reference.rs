use std::cell::RefCell;
use std::rc::Rc;
use crate::holon::{Holon, HolonGetters};
use crate::holon_errors::HolonError;
use hdk::prelude::*;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapString};

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

    fn get_key(&self) -> Option<MapString> {
        let holon = self.get_holon()?;

    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LocalHolonReference {
    pub holon_id: Option<HolonId>,
    pub rc_holon: Option<Rc<RefCell<Holon>>>,
}

impl HolonReferenceFns for LocalHolonReference {
    /// get_holon will return the cached Holon, first retrieving it from the storage tier, if necessary
    fn get_holon(&self) -> Result<Holon, HolonError> {
        let holon_reference = self.clone();
        if let Some(holon) = holon_reference.rc_holon {
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
            rc_holon: None,
        }
    }

    // Constructor function for creating from Holon
    pub fn from_holon(holon: Holon) -> Self {
        Self {
            holon_id: None,
            rc_holon: Some(holon),
        }
    }
    pub fn add_holon_id(&mut self, holon_id: HolonId) -> &mut Self {
        self.holon_id = Some(holon_id);
        self
    }
    pub fn add_holon(&mut self, holon: Holon) -> &mut Self {
        self.rc_holon = Some(holon);
        self
    }
}
