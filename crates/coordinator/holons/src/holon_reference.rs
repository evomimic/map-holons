use std::cell::RefCell;
use std::rc::Rc;
use crate::holon::{Holon, HolonFieldGettable};
use crate::holon_errors::HolonError;
use hdk::prelude::*;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{HolonId, MapString, PropertyValue};
use crate::context::HolonsContext;
use crate::relationship::RelationshipMap;
// If I can operate directly on HolonReferences as if they were Holons, I don't need this Trait
// pub trait HolonReferenceFns {
//     fn get_rc_holon(&self) -> Result<Rc<RefCell<Holon>>, HolonError>;
// }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HolonReference {
    Local(LocalHolonReference),
    // External(ExternalHolonReference),
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LocalHolonReference {
    pub holon_id: Option<HolonId>, // only populated once the Holon has been persisted (and fetched).
    pub rc_holon: Option<Rc<RefCell<Holon>>>, // only populated during building or when holon has been fetched.
}


// impl HolonReferenceFns for HolonReference {
//     fn get_rc_holon(&self) -> Result<Rc<RefCell<Holon>>, HolonError> {
//         match self {
//             HolonReference::Local(holon_reference) => holon_reference.get_rc_holon(),
//         }
//     }
// }

impl HolonFieldGettable for HolonReference {
    // fn get_property_value(
    //     &self,
    //     property_name: &PropertyName,
    // ) -> Result<Option<BaseValue>, HolonError> {
    //     // let holon = self.get_rc_holon()?;
    //     // holon.get_property_value(property_name)
    //     if let HolonReference::Local(local_ref) = self {
    //         local_ref
    //             .rc_holon
    //             .as_ref()
    //             .and_then(|rc_holon| rc_holon.borrow().property_map.get(property_name))
    //     } else {
    //         None
    //     }
    // }

    fn get_property_value(&self, context: &HolonsContext, key: &PropertyName) -> Result<&PropertyValue, HolonError> {
        if let HolonReference::Local(local_ref) = self {
            if let Some(rc_holon) = &local_ref.rc_holon {
                if let Some(value) = rc_holon.borrow().property_map.get(key) {
                    return Ok(value);
                }
            }
        }
        Err(HolonError::HolonNotFound(format!("Property '{}' not found", key)))
    }

    fn get_property_names(&self, context: &HolonsContext) -> Result<Vec<&PropertyName>, HolonError> {
        if let HolonReference::Local(local_ref) = self {
            if let Some(rc_holon) = &local_ref.rc_holon {
                return Ok(rc_holon.borrow().property_map.keys().collect());
            }
        }
        Ok(Vec::new())
    }

    fn get_relationship_map(&self, context: &HolonsContext,) -> Result<&RelationshipMap, HolonError> {
        if let HolonReference::Local(local_ref) = self {
            if let Some(rc_holon) = &local_ref.rc_holon {
                return Ok(&rc_holon.borrow().relationship_map);
            }
        }
        Err(HolonError::HolonNotFound("No Holon found".to_string()))
    }

    fn get_key(&self, context: &HolonsContext,) -> Result<Option<MapString>,HolonError> {
        self.ensure_rc(context);
        let holon = self.get_rc_holon();
        holon.get_key()

    }
}


// impl HolonReferenceFns for LocalHolonReference {
//     /// get_holon will return a shared reference to the cached Holon, first retrieving it from the storage tier, if necessary
//     fn get_rc_holon(&self) -> Result<Rc<RefCell<Holon>>, HolonError> {
//         let holon_reference = self.clone();
//         if let Some(holon) = holon_reference.rc_holon {
//             Ok(holon)
//         } else {
//             if let Some(id) = holon_reference.holon_id {
//                 Holon::fetch_holon(id)
//             } else {
//                 Err(HolonError::HolonNotFound(
//                     "LocalHolonReference is empty".to_string(),
//                 ))
//             }
//         }
//     }
// }

impl LocalHolonReference {
    // Constructor function for creating from HolonId
    pub fn from_holon_id(holon_id: HolonId) -> Self {
        Self {
            holon_id: Some(holon_id),
            rc_holon: None,
        }
    }

    // Constructor function for creating from Holon Reference
    pub fn from_holon(rc_holon: Rc<RefCell<Holon>>) -> Self {
        Self {
            holon_id: None,
            rc_holon: Some(rc_holon),
        }
    }
    pub fn add_holon_id(&mut self, holon_id: HolonId) -> &mut Self {
        self.holon_id = Some(holon_id);
        self
    }
    pub fn add_rc_holon(&mut self, rc_holon: Rc<RefCell<Holon>>) -> &mut Self {
        self.rc_holon = Some(rc_holon);
        self
    }

    /// ensure_rc -- is a private function that attempts to ensure that the HolonReference contains a populated rc_holon
    /// If rc_holon is already populated, it simply returns Ok(self)
    /// Otherwise, if holon_id is populated,
    ///    it will attempt to fetch the holon from the persistent store,
    ///    If found,
    ///        populate rc_holon to refer to the fetched (and cache)
    ///        and return Ok(self)
    ///    Else return HolonNotFound error
    /// If neither rc_holon nor holon_id is populated, return InvalidHolonReference (this should never happen).
    ///
    fn ensure_rc(&mut self, context: &HolonsContext) -> Result<&mut Self, HolonError> {
        if self.rc_holon = None {
            if let Some(id) = self.holon_id.clone() {
                let rc_holon = Holon::fetch_holon(context, id)?;
                self.add_rc_holon(rc_holon);
            } else {
                Err(HolonError::InvalidHolonReference("LocalHolonReference has neither id nor rc".to_string()))
            }
        }
        Ok(self)

    }

}
