use hdi::prelude::info;
use shared_types_holon::{HolonId, MapString};
use crate::commit_manager::CommitManager;

use crate::context::HolonsContext;
use crate::holon::Holon;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::holon_space::HolonSpace;
use crate::smart_reference::SmartReference;

pub struct HolonSpaceManager<'a> {
    context: &'a HolonsContext,  // Reference to the context where the HolonSpace will be persisted
}

impl<'a> HolonSpaceManager<'a> {
    // Constructor for the service
    pub fn new(context: &'a HolonsContext) -> Self {
        HolonSpaceManager { context }
    }
    /// This function stages and commits a HolonSpace within this DHT
    /// It is intended to be called from the init() function with _*exactly-once*_ semantics being
    /// enforced by the progenitor pattern.
    pub fn create_local_holon_space(&self) -> Result<HolonReference, HolonError> {
        info!("Preparing to stage and commit the LocalHolonSpace");
        let mut holon_space = HolonSpace::new(Holon::new());
        let name = MapString("LocalHolonSpace".to_string());
        let description = MapString("Default HolonSpace description. Actual description should be \
        loaded from DNA properties.".to_string());

        holon_space
            .with_name(&name)?
            .with_description(&description)?
        ;

        // Stage the new holon space and set it in the context
        let _staged_holon_space_ref = self.context
            .commit_manager
            .borrow_mut()
            .stage_new_holon(holon_space.into_holon())?;

        // Commit the staged holon space
        let commit_response = CommitManager::commit(self.context);

        if commit_response.is_complete() {
            let local_id =commit_response.find_local_id_by_key(&name)?;
            info!("Created LocalHolonSpace with id {:#?}", local_id.clone());
            return Ok(HolonReference::Smart(SmartReference::new_from_id(local_id.into())))
            // TODO: Tie the newly saved holon to the holon space LinkType anchor
        }
        return Err(HolonError::CommitFailure("Unable to commit LocalHolonSpace".to_string()))

    }
    /// Ensure that a LocalHolonSpace reference is included in the context. The simplest case is
    /// that context is already populated with the reference. If not, try to fetch the reference
    /// from the persistent store. If that doesn't work, then (for now), try to stage and commit
    /// the local HolonSpace.
    ///
    pub fn ensure_local_holon_space_in_context(&self)-> Result<HolonReference, HolonError> {
        return match self.context.get_local_holon_space() {
            Some(space_reference) => Ok(space_reference),
            None => {
                info!("No LocalHolonSpace found in context, fetching it.");
                let holon_space_fetch_result = self.fetch_and_set_local_holon_space();
                match holon_space_fetch_result {
                    Ok(space_reference) => {
                        self.context.set_local_holon_space(space_reference.clone())?;
                        Ok(space_reference)
                    },
                    Err(_fetch_error) => {
                        // Handle the case where we were unable to get the LocalHolonSpace from the
                        // persistent store.
                        // NOTE: Once we have moved holon space creation to init(), we should just
                        // return an error indicating initialization is not complete.
                        // But for now, use this to trigger creation of the local holon space
                        let space_reference = self.create_local_holon_space()?;
                        self.context.set_local_holon_space(space_reference.clone())?;
                        Ok(space_reference)
                    },
                }
            }
        }
    }

    /// Search the DHT for its (singleton) LocalHolonSpace and update the context to include
    /// a HolonReference to it. Returns a HolonNotFound error if LocalHolonSpace cannot be found.
    fn fetch_and_set_local_holon_space(&self) -> Result<HolonReference, HolonError> {
        // For now, it just uses a brute force linear search through all saved holons, searching
        // for a holon with key "LocalHolonSpace". If found, it extracts its HolonId and
        // a HolonReference for that HolonId is returned
        // TODO: Scaffold a new `LocalHolonSpace` LinkType and search by path instead of linear search
        let all_holons = Holon::get_all_holons()?;
        let search_key = MapString("LocalHolonSpace".to_string());

        for holon in &all_holons {
            match holon.get_key()? {
                Some(key) if key == search_key => {
                    let holon_id = HolonId::Local(holon.get_local_id()?);
                    let holon_space_reference = HolonReference::Smart(SmartReference::new_from_id(holon_id));
                    self.context.set_local_holon_space(holon_space_reference.clone())?;
                    return Ok(holon_space_reference);
                }
                _ => continue,
            }
        }
        // Return HolonError::NotFound if no matching holon is found
        Err(HolonError::HolonNotFound(search_key.to_string()))
    }
}