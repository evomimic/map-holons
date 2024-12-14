use hdi::prelude::*;
use holons_integrity::LinkTypes;
use quick_cache::unsync::Cache;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::commit_service::{CommitRequestStatus, CommitResponse, CommitService};
use crate::context::HolonsContext;
use shared_types_holon::{HolonId, LocalId, MapString};

use crate::holon::{Holon, HolonState};
use crate::holon_collection::CollectionState;
use crate::holon_error::HolonError;

use crate::holon_reference::HolonReference;
use crate::holon_service;
use crate::nursery::{Nursery, NurseryBehavior};
use crate::staged_reference::{StagedIndex, StagedReference};

#[derive(Debug, Clone)]
pub struct HolonCache(Cache<HolonId, Rc<RefCell<Holon>>>);

#[derive(Debug, Clone)]
pub struct HolonSpaceManager {
    //hashtable<proxy>
    space: Option<Holon>,
    space_ref: Option<HolonReference>,
    cache: Rc<RefCell<HolonCache>>, //add Rc<RefCell<  //already committed holons
    nursery: RefCell<Nursery>,
}
pub trait HolonCacheBehavior {
    /// This method returns a mutable reference (Rc<RefCell>) to the Holon identified by holon_id.
    /// If holon_id is `Local`, it retrieves the holon from the local cache. If the holon is not
    /// already resident in the cache, this function first fetches the holon from the persistent
    /// store and inserts it into the cache before returning the reference to that holon.
    /// If the holon_id is `External`, this method currently returns a `NotImplemented` HolonError
    ///
    /// TODO: Enhance to support `External` HolonIds
    ///
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError>;

    /// This method adds the provided holons to the cache
    fn add_to_cache(&self, holons: Vec<Holon>) -> Result<(), HolonError>;
}


///direct access to holons
pub trait HolonStageQuery {
    /// holon from a StageReference
    fn get_holon(&self, reference: &StagedReference) -> Result<Rc<RefCell<Holon>>, HolonError>;
    /// holon from a Stage index
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError>;
    fn get_all_holons(&self) -> Vec<Rc<RefCell<Holon>>>;
}

///comon stage operations
///direct access to holons
pub trait HolonStageQuery {
    /// holon from a StageReference
    fn get_holon(&self, reference: &StagedReference) -> Result<Rc<RefCell<Holon>>, HolonError>;
    /// holon from a Stage index
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError>;
    fn get_staged_holons(&self) -> Vec<Rc<RefCell<Holon>>>;
}

///comon stage operations
pub trait HolonStagingBehavior {
    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the staged holon
    /// to be retrieved by key
    fn stage_new_holon(&self, holon: Holon) -> Result<StagedReference, HolonError>;

    /// This function converts a StagedIndex into a StagedReference
    /// Returns HolonError::IndexOutOfRange if index is out range for staged_holons vector
    /// Returns HolonError::NotAccessible if the staged holon is in an Abandoned state
    fn to_staged_reference(&self,staged_index: StagedIndex) -> Result<StagedReference, HolonError>;
    /// Returns a dictionary indexed by key of all staged holons
    fn to_staged_reference(&self, staged_index: StagedIndex)
        -> Result<StagedReference, HolonError>;
    /// Returns a dictionary indexed by key of all staged holons
    fn get_stage_key_index(&self) -> BTreeMap<MapString, usize>;
    ///holon reference from a key name
    fn get_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError>;
    //fn get_mut_holon_by_index(&self, holon_index: StagedIndex) -> Result<RefMut<Holon>, HolonError>
}

impl HolonSpaceManager {
    pub fn new() -> Self {
        // Initialize local cache
        let cache = Cache::new(99);
        let local_cache = HolonCache(cache);
        HolonSpaceManager {
            space: None,
            space_ref: None,
            cache: Rc::new(local_cache.into()),
            nursery: RefCell::new(Nursery::new()),
        }
    }

    pub fn new_from_session(
        staged_holons: Vec<Rc<RefCell<Holon>>>,
        keyed_index: BTreeMap<MapString, usize>,
        space_holon_ref: Option<HolonReference>,
    ) -> Self {
        let nursery = Nursery::new_from_stage(staged_holons, keyed_index);
        HolonSpaceManager {
            space: None,
            space_ref: space_holon_ref,
            cache: Rc::new(HolonCache(Cache::new(99)).into()),
            nursery: RefCell::new(nursery),
        }
    }

    pub fn get_space_holon(&self) -> Option<HolonReference> {
        self.space_ref.clone()
    }

    pub fn set_space_holon(&mut self, space: Holon) {
        self.space = Some(space);
    }

    pub fn set_space_holon_ref(&mut self, space: HolonReference) {
        self.space_ref = Some(space);
    }

    /// Ensure that a Local Space Holon reference is found. The simplest case is
    /// that session is already populated with the reference. If not, try to fetch the reference
    /// from the persistent store. If that doesn't work, then stage and commit the local HolonSpace.
    pub fn ensure_local_holon_space(
        &mut self,
        context: &HolonsContext,
    ) -> Result<HolonReference, HolonError> {
        return match self.get_space_holon() {
            Some(space_reference) => Ok(space_reference),
            None => {
                debug!("No Local Space Holon found in session, fetching it.");
                let spaceholon = holon_service::get_holon_by_path(
                    "local_holon_space".to_string(),
                    LinkTypes::LocalHolonSpace,
                );
                match spaceholon {
                    Ok(spaceholon) => {
                        match spaceholon {
                            Some(spaceholon) => {
                                debug!("Local Space holon found in storage, setting references.");
                                let holon_id = HolonId::Local(spaceholon.get_local_id()?);
                                let ref_spaceholon =
                                    HolonReference::smartreference_from_holon_id(holon_id);
                                self.set_space_holon(spaceholon);
                                self.set_space_holon_ref(ref_spaceholon.clone());
                                return Ok(ref_spaceholon);
                            }
                            None => {
                                info!(
                                    "Local Space Holon not found in storage, creating a new one."
                                );
                                let mut uncommited_space_holon = Holon::new();
                                let name = MapString("LocalHolonSpace".to_string());
                                let description =
                                    MapString("Default Local Holon Space".to_string());
                                uncommited_space_holon
                                    .with_name(&name)?
                                    .with_description(&description)?;
                                let _stage_result =
                                    self.stage_new_holon(uncommited_space_holon.clone())?;
                                // COMMIT
                                let commit_response = self.commit(context)?;
                                if commit_response.is_complete() {
                                    let local_id = commit_response.find_local_id_by_key(&name)?;
                                    let holon_id = HolonId::Local(local_id.clone());
                                    info!(
                                        "Created LocalHolonSpace with id {:#?}",
                                        local_id.clone()
                                    );
                                    self.set_space_holon(commit_response.saved_holons[0].clone());
                                    let create_local_path = holon_service::create_local_path(
                                        local_id,
                                        "local_holon_space".to_string(),
                                        LinkTypes::LocalHolonSpace,
                                    );
                                    match create_local_path {
                                        Ok(_) => {
                                            let space_reference =
                                                HolonReference::smartreference_from_holon_id(
                                                    holon_id,
                                                );
                                            self.set_space_holon_ref(space_reference.clone());
                                            return Ok(space_reference);
                                        }
                                        Err(_) => {
                                            return Err(HolonError::CommitFailure(
                                                "Unable to crate path to the LocalHolonSpace"
                                                    .to_string(),
                                            ));
                                        }
                                    }
                                } else {
                                    return Err(HolonError::CommitFailure(
                                        "Unable to commit LocalHolonSpace".to_string(),
                                    ));
                                }
                            }
                        }
                    }
                    Err(fetch_error) => return Err(fetch_error),
                }
            }
        };
    }

    pub fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        if let Some(ref holon) = self.space {
            holon.get_key()
        } else {
            Err(HolonError::NotAccessible("get_key".to_string(), "No space holon".to_string()))
        }
    }

    pub fn get_local_id(&self) -> Result<LocalId, HolonError> {
        if let Some(ref holon) = self.space {
            holon.get_local_id()
        } else {
            Err(HolonError::NotAccessible("get_local_id".to_string(), "No space holon".to_string()))
        }
    }

    // fn holon_mut(&mut self) -> Option<&mut Holon> {
    //         self.space.as_mut() // Return a mutable reference to the inner `Holon`
    //    }

    pub fn into_holon(self) -> Holon {
        self.space.expect("No space holon found").clone()
    }

    /// This function commits the staged holons to the persistent store
    pub fn commit(&self, context: &HolonsContext) -> Result<CommitResponse, HolonError> {
        //TODO here we could lock the stage to prevent writes but allow reads
        let response = CommitService::commit(self, context)?; //&self,context
        if response.status == CommitRequestStatus::Complete {
            //Optional TODO to avoid dht ops: Uncomment code and add saved holons to cache
            //self.add_to_cache(response.saved_holons.clone())?;
            self.nursery.borrow_mut().clear_stage();
            Ok(response)
        } else {
            Ok(response)
        }
    }
}

impl HolonStageQuery for HolonSpaceManager {
    fn get_holon(&self, reference: &StagedReference) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.nursery.borrow().get_holon_by_index(reference.holon_index)
    }
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.nursery.borrow().get_holon_by_index(index)
    }
    fn get_staged_holons(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.nursery.borrow().get_all_holons()
    }
}


impl HolonStageQuery for HolonSpaceManager {
    fn get_holon(&self, reference: &StagedReference) -> Result<Rc<RefCell<Holon>>, HolonError> {
        self.nursery.borrow().get_holon_by_index(reference.holon_index)
    }
     fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError>{
        self.nursery.borrow().get_holon_by_index(index)
    }
    fn get_all_holons(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.nursery.borrow().get_all_holons()
    }
}

impl HolonStagingBehavior for HolonSpaceManager {
    fn stage_new_holon(&self, mut holon: Holon) -> Result<StagedReference, HolonError> {
        //let mut cloned_holon = holon.clone();
        for (_relationship_name, collection) in holon.relationship_map.0.iter_mut() {
            let state = collection.get_state();
            match state {
                CollectionState::Fetched => {
                    collection.to_staged()?;
                }
                CollectionState::Staged => {}
                CollectionState::Saved | CollectionState::Abandoned => {
                    return Err(HolonError::InvalidParameter(format!(
                        "CollectionState::{:?}",
                        state
                    )))
                }
            }
        }
        let holon_index = self.nursery.borrow_mut().add_new_holon(holon)?;
        Ok(StagedReference { holon_index })
    }

    fn to_staged_reference(
        &self,
        staged_index: StagedIndex,
    ) -> Result<StagedReference, HolonError> {
        if let Ok(staged_holon) = self.nursery.borrow().get_holon_by_index(staged_index) {
            let holon = &staged_holon.borrow();
            if let HolonState::Abandoned = holon.state {
                return Err(HolonError::NotAccessible(
                    "to_staged_reference".to_string(),
                    "Abandoned".to_string(),
                ));
            }
            Ok(StagedReference { holon_index: staged_index })
        } else {
            Err(HolonError::IndexOutOfRange(staged_index.to_string()))
        }
    }

    fn get_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError> {
        let index = self.nursery.borrow().get_holon_index_by_key(key)?;
        Ok(StagedReference::new(index))
    }

    fn get_stage_key_index(&self) -> BTreeMap<MapString, usize> {
        self.nursery.borrow().get_stage_key_index()
    }
}

impl HolonCacheBehavior for HolonSpaceManager {
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError> {
        let cache = Rc::clone(&self.cache);

        // Attempt to borrow the cache
        let try_cache_borrow = cache.try_borrow().map_err(|e| {
            HolonError::FailedToBorrow(format!("Unable to borrow holon cache immutably: {}", e))
        })?;

        // Check if the holon is already in the cache
        debug!("Checking the cache for local_id: {:#?}", holon_id.local_id());
        if let Some(holon) = try_cache_borrow.0.get(holon_id) {
            return Ok(Rc::clone(holon));
        }
        drop(try_cache_borrow);

        // Holon not found in cache, fetch it
        debug!("Holon not cached, fetching holon");
        let fetched_holon = holon_service::get_holon_by_id(holon_id)?;

        // Attempt to borrow the cache mutably
        let mut cache_mut = cache.try_borrow_mut().map_err(|e| {
            HolonError::FailedToBorrow(format!("Unable to borrow_mut holon cache: {}", e))
        })?;

        // Insert the fetched holon into the cache
        debug!(
            "Inserting fetched holon into cache for local_id: {:#?}",
            fetched_holon.get_local_id(),
        );
        cache_mut.0.insert(holon_id.clone(), Rc::new(RefCell::new(fetched_holon)));

        // Return a new Rc<RefCell<Holon>> containing the fetched holon
        Ok(Rc::clone(cache_mut.0.get(holon_id).expect("Holon should be present in the cache")))
    }

    fn add_to_cache(&self, holons: Vec<Holon>) -> Result<(), HolonError> {
        let cache = Rc::clone(&self.cache);

        // Attempt to borrow the cache mutably
        let mut cache_mut = cache.try_borrow_mut().map_err(|e| {
            HolonError::FailedToBorrow(format!("Unable to borrow_mut holon cache: {}", e))
        })?;
        for holon in holons {
            let holon_id = HolonId::Local(holon.get_local_id()?);
            cache_mut.0.insert(holon_id.clone(), Rc::new(RefCell::new(holon)));
        }
        Ok(())
    }
}
