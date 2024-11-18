use hdi::prelude::{info, ActionHash, Path};
use holons_integrity::LinkTypes;
use shared_types_holon::{HolonId, LocalId, MapString};

use crate::context::HolonsContext;
use crate::holon::Holon;
use crate::holon_error::HolonError;
use crate::holon_node::{
    create_path_to_holon_node, get_holon_node_by_path, CreatePathInput, GetPathInput,
};
use crate::holon_reference::HolonReference;
use crate::smart_reference::SmartReference;

//TODO The holonSpaceManager manages the of lifetime of HolonSpaces and should keep a vector of references to HolonSpaces?, 
pub struct LocalContextService<'a> {
    context: &'a HolonsContext, // Reference to the context where the HolonSpace will be persisted
}

impl<'a> LocalContextService<'a> {
    // Constructor for the service
    pub fn new(context: &'a HolonsContext) -> Self {
        LocalContextService { context }
    }
    /// This function stages and commits a HolonSpace within this DHT
    /// It is intended to be called from the init() function with _*exactly-once*_ semantics being
    /// enforced by the progenitor pattern.
    pub fn create_local_holon_space(&self) -> Result<HolonReference, HolonError> {
        info!("Preparing to stage and commit the Local Space Holon");
        let space_holon = Holon::new();
        let name = MapString("LocalHolonSpace".to_string());
        let description = MapString(
            "Default HolonSpace description. Actual description should be \
        loaded from DNA properties."
                .to_string(),
        );
        let mut space_manager = self.context.local_space_manager.borrow_mut();
        space_manager.set_space_holon(space_holon);
        space_manager.with_name(&name)?.with_description(&description)?;

        let commit_response = space_manager.commit_stage(self.context)?;//&self.context.commit_manager.borrow())?;

        // Stage the new holon space and set it in the context
        //let _staged_holon_space_ref =
           // self.context.commit_manager.borrow_mut().stage_new_holon(holon_space.into_holon())?;

        // Commit the staged holon space
       // let commit_response = CommitManager::commit(self.context);

        if commit_response.is_complete() {
            let local_id = commit_response.find_local_id_by_key(&name)?;
            info!("Created LocalHolonSpace with id {:#?}", local_id.clone());

            LocalContextService::create_local_path(local_id.clone()).map_err(|e| {
                return HolonError::CommitFailure(
                    "Unable to create LocalHolonSpace path, inner error: ".to_string()
                        + &e.to_string(),
                );
            })?;

            return Ok(HolonReference::Smart(SmartReference::new_from_id(local_id.into())));
        }
        return Err(HolonError::CommitFailure("Unable to commit LocalHolonSpace".to_string()));
    }

    fn create_local_path(target_holon_hash: LocalId) -> Result<ActionHash, HolonError> {
        let path = Path::from("local_holon_space");
        let link_type = LinkTypes::LocalHolonSpace;
        let input = CreatePathInput {
            path: path,
            link_type: link_type,
            target_holon_node_hash: target_holon_hash.0,
        };
        create_path_to_holon_node(input).map_err(|e| HolonError::from(e))
    }

    fn fetch_local_holon_space() -> Result<Holon, HolonError> {
        let path = Path::from("local_holon_space");
        let link_type = LinkTypes::LocalHolonSpace;
        let input = GetPathInput { path: path.clone(), link_type: link_type };
        let record = get_holon_node_by_path(input)
            .map_err(|e| HolonError::from(e))?
            .ok_or_else(|| HolonError::HolonNotFound(format!("at path: {:?}", path)))?;
        Ok(Holon::try_from_node(record)?)
    }
    /// Ensure that a LocalHolonSpace reference is included in the context. The simplest case is
    /// that context is already populated with the reference. If not, try to fetch the reference
    /// from the persistent store. If that doesn't work, then (for now), try to stage and commit
    /// the local HolonSpace.
    ///
    pub fn ensure_local_space_holon_in_context(&self) -> Result<HolonReference, HolonError> {
        return match self.context.get_local_space_holon() {
            Some(space_reference) => Ok(space_reference),
            None => {
                info!("No LocalHolonSpace found in context, fetching it.");
                let holon_space_fetch_result = self.fetch_and_set_local_holon_space();
                match holon_space_fetch_result {
                    Ok(space_reference) => {
                        self.context.set_local_space_holon(space_reference.clone())?;
                        Ok(space_reference)
                    }
                    Err(_fetch_error) => {
                        // Handle the case where we were unable to get the LocalHolonSpace from the
                        // persistent store.
                        // NOTE: Once we have moved holon space creation to init(), we should just
                        // return an error indicating initialization is not complete.
                        // But for now, use this to trigger creation of the local holon space
                        let space_reference = self.create_local_holon_space()?;
                        self.context.set_local_space_holon(space_reference.clone())?;
                        Ok(space_reference)
                    }
                }
            }
        };
    }

    /// Search the DHT for its (singleton) LocalHolonSpace and update the context to include
    /// a HolonReference to it. Returns a HolonNotFound error if LocalHolonSpace cannot be found.
    fn fetch_and_set_local_holon_space(&self) -> Result<HolonReference, HolonError> {
        let holon = Self::fetch_local_holon_space().or_else(|e| return Err(e))?;
        let holon_id = HolonId::Local(holon.get_local_id()?);
        let holon_space_reference = HolonReference::Smart(SmartReference::new_from_id(holon_id));
        self.context.set_local_space_holon(holon_space_reference.clone())?;
        Ok(holon_space_reference)//_reference);
    }
}
