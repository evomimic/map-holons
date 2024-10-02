use hdi::map_extern::ExternResult;
use hdi::prelude::Path;
use hdk::hdk::HDK;
use hdk::link::{get_links, GetLinksInputBuilder};
use hdk::prelude::{GetInput, GetOptions};
use holochain_integrity_types::Record;
use holons_integrity::LinkTypes;
use shared_types_holon::{HolonId, MapString};
use crate::context::HolonsContext;
use crate::holon::Holon;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::smart_reference::SmartReference;


pub struct HolonSpace(pub Holon);

impl HolonSpace {
    pub fn new(holon: Holon) -> HolonSpace {
        HolonSpace(holon)
    }

    pub fn into_holon(self) -> Holon {
        self.0
    }
    /// get_local_holon_space retrieves the local holon space from the persistent store
    pub fn get_local_holon_space(context: &HolonsContext) -> Result<HolonReference, HolonError> {
        // For now, it just uses a brute force linear search through all saved holons, searching
        // for a holon with key "LocalHolonSpace". If found, it extracts its HolonId and
        // A HolonReference to the cached HolonSpace Holon is then returned
        // TODO: Scaffold a new `LocalHolonSpace` LinkType and search by path instead of linear search
        let all_holons = Holon::get_all_holons()?;
        let search_key = MapString("LocalHolonSpace".to_string());

        for holon in &all_holons {
            match holon.get_key()? {
                Some(key) if key == search_key  => {
                    let holon_id = HolonId::Local(holon.get_local_id()?);
                    // use get_rc_holon on the cache_manager to populate the HolonSpace holon in the cache
                    let holon_space_rc_holon = context
                        .cache_manager
                        .borrow_mut()
                        .get_rc_holon(&holon_id)?;
                    // build a HolonReference from the holon_space_rc_holon.
                    // TODO: We should have a helper function that creates a new SmartReference from a Holon.
                    // This function could populate the smart_property_values from the holon's descriptor
                    // But for now, we'll just construct the HolonReference from holon_id

                    return Ok(HolonReference::Smart(SmartReference::new(holon_id, None)));
                }
                _ => continue,
            }
        }

        // Return HolonError::NotFound if no matching holon is found
        Err(HolonError::HolonNotFound(search_key.to_string()))
    }


}
