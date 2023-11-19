// use hdk::entry::{get, hdk_entry_helper};
use hdk::prelude::*;
use crate::holon_errors::HolonError;
use crate::holon;
use crate::holon::{Holon};




#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq )]
pub enum HolonReference {
    Local(LocalHolonReference),
    // External(ExternalHolonReference),
}
pub trait HolonReferenceFns {
    fn get_holon(self)->Result<Holon,HolonError>;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq )]
pub struct LocalHolonReference {
    holon_id: ActionHash,
}
impl HolonReferenceFns for LocalHolonReference {
    // get_holon retrieves the holon for a HolonReference
    // currently, always does a fetch,
    // future: retrieve from cache
    fn get_holon(self)->Result<Holon, HolonError> {
       holon::fetch_holon(self.holon_id)

    }
}
