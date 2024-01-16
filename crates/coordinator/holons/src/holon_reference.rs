use hdk::prelude::*;
use crate::holon_errors::HolonError;
use crate::holon_types::Holon;
// use crate::holon::*;


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
    holon_id: Option<ActionHash>,
    holon: Option<Holon>,
}

impl LocalHolonReference {
    pub fn new() -> LocalHolonReference {
        LocalHolonReference {
            holon_id : None,
            holon: None,
        }
    }
    pub fn with_holon(&mut self, holon:Holon) -> &mut Self{
        self.holon = Some(holon);
        self
    }


}

// TODO: figure out why fetch_holon function can't be found in the following
// impl HolonReferenceFns for LocalHolonReference {
//     // get_holon retrieves the holon for a HolonReference
//     // currently, always does a fetch,
//     // future: retrieve from cache
//     fn get_holon(self)->Result<Holon, HolonError> {
//       fetch_holon(self.holon_id)
//
//     }
// }
