use crate::{delete_holon_node, holon_service, CommitService, LocalHolonResolver};
use hdi::prelude::*;
use hdk::entry::get;
use hdk::prelude::GetOptions;
use holons_core::cache_access::HolonCacheAccess;
use holons_core::nursery_access::NurseryAccess;
use holons_core::staged_reference::StagedIndex;
use holons_core::{
    CommitResponse, Holon, HolonCacheManager, HolonError, HolonReference, HolonSpaceBehavior,
    HolonStagingBehavior, HolonState, HolonsContextBehavior, Nursery, StagedReference,
    StateMobility,
};
use holons_integrity::LinkTypes;
use shared_types_holon::{HolonId, LocalId, MapString};
use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct GuestHolonSpaceManager {
    local_holon_space: Option<HolonReference>,
    local_cache_manager: HolonCacheManager, // Manages the local cache
    nursery: RefCell<Nursery>,
    commit_service: CommitService,
}

impl GuestHolonSpaceManager {
    pub fn new() -> Self {
        // Initialize local cache manager

        let local_cache_manager = HolonCacheManager::new(Arc::new(LocalHolonResolver));
        GuestHolonSpaceManager {
            local_holon_space: None,
            local_cache_manager,
            nursery: RefCell::new(Nursery::new()),
            commit_service: CommitService::new(),
        }
    }

    pub fn new_from_session(
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
        space_holon_ref: Option<HolonReference>,
    ) -> Self {
        let nursery = Nursery::new_from_stage(staged_holons, keyed_index);
        let local_cache_manager = HolonCacheManager::new(Arc::new(LocalHolonResolver));
        GuestHolonSpaceManager {
            local_holon_space: space_holon_ref,
            local_cache_manager,
            nursery: RefCell::new(nursery),
            commit_service: CommitService::new(),
        }
    }

    pub fn set_space_holon(&mut self, space: HolonReference) {
        self.local_holon_space = Some(space);
    }

    /// Ensure that a Local Space Holon reference is found.
    ///
    /// This function checks if the session already contains the reference to the Local Space Holon.
    /// If not, it attempts to fetch the reference from the persistent store. If that also fails,
    /// a new Local Space Holon is created, staged, and committed.
    ///
    /// # Arguments
    ///
    /// * `context` - A reference to the context, providing access to required managers.
    ///
    /// # Returns
    ///
    /// `Ok(HolonReference)` containing the Local Space Holon reference if successful.
    /// `Err(HolonError)` if any errors occur during the process.
    pub fn ensure_local_holon_space(
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonReference, HolonError> {
        if let Some(space_reference) = self.get_space_holon() {
            return Ok(space_reference);
        }

        debug!("No Local Space Holon found in session, attempting to fetch from storage.");

        let space_reference = match holon_service::get_holon_by_path(
            "local_holon_space".to_string(),
            LinkTypes::LocalHolonSpace,
        )? {
            Some(space_holon) => {
                debug!("Local Space Holon found in storage.");
                let holon_id = HolonId::Local(space_holon.get_local_id()?);
                HolonReference::holon_reference_from_id(holon_id)
            }
            None => {
                info!("Local Space Holon not found in storage, creating a new one.");
                self.create_and_commit_local_space_holon(context)?
            }
        };

        self.set_space_holon(space_reference.clone());
        Ok(space_reference)
    }

    /// Helper function to create, stage, and commit a new Local Space Holon.
    ///
    /// This function is called when no Local Space Holon exists in either the session or the store.
    ///
    /// # Arguments
    ///
    /// * `context` - A reference to the context, providing access to required managers.
    ///
    /// # Returns
    ///
    /// `Ok(HolonReference)` containing the reference to the newly created Local Space Holon if successful.
    /// `Err(HolonError)` if any errors occur during the process.
    fn create_and_commit_local_space_holon(
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonReference, HolonError> {
        let mut uncommitted_space_holon = Holon::new();
        let name = MapString("LocalHolonSpace".to_string());
        let description = MapString("Default Local Holon Space".to_string());

        uncommitted_space_holon.with_name(&name)?.with_description(&description)?;

        self.stage_new_holon(uncommitted_space_holon)?;

        let commit_response = self.commit(context)?;

        if !commit_response.is_complete() {
            return Err(HolonError::CommitFailure("Unable to commit LocalHolonSpace".to_string()));
        }

        let local_id = commit_response.find_local_id_by_key(&name)?;
        let holon_id = HolonId::Local(local_id.clone());
        info!("Created LocalHolonSpace with id {:#?}", local_id);

        holon_service::create_local_path(
            local_id,
            "local_holon_space".to_string(),
            LinkTypes::LocalHolonSpace,
        )?;

        Ok(HolonReference::holon_reference_from_id(holon_id))
    }
}

impl NurseryAccess for GuestHolonSpaceManager {
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.nursery.borrow().get_holon_by_index(index)
    }
}
impl HolonSpaceBehavior for GuestHolonSpaceManager {
    /// Allows dynamic downcasting by exposing a `dyn Any` reference.
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn delete_holon(&self, local_id: &LocalId) -> Result<(), HolonError> {
        let record = get(local_id.0.clone(), GetOptions::default())
            .map_err(HolonError::from)?
            .ok_or_else(|| HolonError::HolonNotFound(format!("at id: {:?}", local_id.0)))?;
        let mut holon = Holon::try_from_node(record)?;
        holon.is_deletable()?;
        delete_holon_node(local_id.0.clone())
            .map(|_| ()) // Convert ActionHash to ()
            .map_err(HolonError::from)
    }

    fn get_space_holon(&self) -> Option<HolonReference> {
        self.local_holon_space.clone()
    }
}

impl HolonStagingBehavior for GuestHolonSpaceManager {
    fn commit(&self, context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError> {
        let nursery_ref = self.nursery.borrow(); // Create a longer-lived binding for the borrow
        let staged_holons = nursery_ref.get_staged_holons(); // Use the longer-lived borrow
        let commit_response = self.commit_service.commit(context, staged_holons)?;
        drop(nursery_ref); // Explicitly drop the borrow if necessary before borrowing mutably
        self.nursery.borrow_mut().clear_stage(); // Mutably borrow nursery after the previous borrow ends
        Ok(commit_response)
    }

    fn get_staged_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError> {
        let index = self.nursery.borrow().get_index_by_key(&key)?;
        self.to_validated_staged_reference(index)
    }

    fn stage_new_holon(&self, holon: Holon) -> Result<StagedReference, HolonError> {
        // Borrow the nursery mutably and stage the holon
        let staged_index = self.nursery.borrow_mut().stage_holon(&holon);

        // Convert the staged index into a StagedReference
        self.to_validated_staged_reference(staged_index)
    }

    fn to_validated_staged_reference(
        &self,
        staged_index: StagedIndex,
    ) -> Result<StagedReference, HolonError> {
        if let Ok(staged_holon) = self.nursery.borrow().get_holon_by_index(staged_index) {
            let holon = &staged_holon.borrow();
            if let HolonState::Abandoned = holon.state {
                return Err(HolonError::NotAccessible(
                    "to_validated_staged_reference".to_string(),
                    "Abandoned".to_string(),
                ));
            }
            Ok(StagedReference::from_index(staged_index))
        } else {
            Err(HolonError::IndexOutOfRange(staged_index.to_string()))
        }
    }
}

impl HolonCacheAccess for GuestHolonSpaceManager {
    /// Retrieves a mutable reference (Rc<RefCell>) to the Holon identified by `holon_id`.
    /// Delegates to the `local_cache_manager` for `Local` HolonIds, and returns `NotImplemented`
    /// for `External` HolonIds.
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        match holon_id {
            HolonId::Local(local_id) => {
                // Delegate to the local_cache_manager for LocalId
                self.local_cache_manager.get_holon(local_id)
            }
            HolonId::External(_) => {
                // Return NotImplemented error for ExternalId
                Err(HolonError::NotImplemented(
                    "Resolution of External HolonIds is not yet supported.".to_string(),
                ))
            }
        }
    }
    // fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError> {
    //     let cache = self.get_cache()?;
    //
    //     // Check if the holon is already in the cache
    //     debug!("Checking the cache for local_id: {:#?}", holon_id.local_id());
    //     if let Some(holon) = cache.get(holon_id) {
    //         return Ok(Rc::clone(holon));
    //     }
    //     drop(try_cache_borrow);
    //
    //     // Holon not found in cache, fetch it
    //     debug!("Holon not cached, fetching holon");
    //     let fetched_holon = holon_service::get_holon_by_id(holon_id)?;
    //
    //     // Attempt to borrow the cache mutably
    //     let mut cache_mut = cache.try_borrow_mut().map_err(|e| {
    //         HolonError::FailedToBorrow(format!("Unable to borrow_mut holon cache: {}", e))
    //     })?;
    //
    //     // Insert the fetched holon into the cache
    //     debug!(
    //         "Inserting fetched holon into cache for local_id: {:#?}",
    //         fetched_holon.get_local_id(),
    //     );
    //     cache_mut.0.insert(holon_id.clone(), Rc::new(RefCell::new(fetched_holon)));
    //
    //     // Return a new Rc<RefCell<Holon>> containing the fetched holon
    //     Ok(Rc::clone(cache_mut.0.get(holon_id).expect("Holon should be present in the cache")))
    // }
}

impl StateMobility for GuestHolonSpaceManager {
    fn export_staged_holons(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.nursery.borrow().get_staged_holons().clone()
    }

    fn export_keyed_index(&self) -> BTreeMap<MapString, usize> {
        self.nursery.borrow().get_keyed_index()
    }
    /// fetch_holon returns the Holon (NOT a reference to the Holon) for the given HolonId
    /// It is ONLY intended to support the fetch_holon dance that allows MAP client caches to
    /// resolve cache misses.
    fn fetch_holon(&self, id: HolonId) -> Result<Holon, HolonError> {
        let rc_holon = self.get_rc_holon(&id)?; // Get the Rc<RefCell<Holon>>

        // Borrow the RefCell immutably to access the Holon and clone it
        let holon = rc_holon
            .try_borrow()
            .map_err(|e| HolonError::FailedToBorrow(format!("Failed to borrow holon: {}", e)))?
            .clone();

        Ok(holon)
    }
}
