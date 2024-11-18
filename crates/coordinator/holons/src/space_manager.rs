use std::borrow::{Borrow, BorrowMut};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::rc::Rc;
use quick_cache::unsync::Cache;
use hdi::prelude::{Deserialize, Path, Serialize};

use shared_types_holon::{HolonId, LocalId, MapString, PropertyName, PropertyValue};
use shared_types_holon::HolonId::{External, Local};

use crate::commit_service::{CommitRequestStatus, CommitResponse, CommitService};
use crate::context::HolonsContext;
use crate::holon::{self, Holon, HolonState};
use crate::holon_collection::CollectionState;
use crate::holon_error::HolonError;
use crate::holon_node::{
    create_path_to_holon_node, get_holon_node_by_path, CreatePathInput, GetPathInput,
};
use crate::nursery::{HolonsNursery, Nursery};
use crate::staged_reference::StagedReference;

pub type StagedIndex = usize;

#[derive(Debug, Clone)]
pub struct HolonCache(Cache<HolonId, Rc<RefCell<Holon>>>);

#[derive(Debug, Clone)]
pub struct SpaceManager {
    space: Option<Holon>,
    cache: HolonCache, //already committed holons
    nursery: Nursery,
   // staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
   // keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}

impl HolonsNursery for Nursery {}

impl SpaceManager {
    pub fn new() -> Self {
        // Initialize local cache
        let cache = Cache::new(99);
        let local_cache = HolonCache(cache);
        SpaceManager{
            space:None,  
            cache:local_cache, 
            nursery:Nursery::new()}
    }

    //self reflective accessor and mutator functions
    pub fn new_from_stage(staged_holons:Vec<Rc<RefCell<Holon>>>, keyed_index:BTreeMap<MapString, usize>)-> Self{
        let nursery = Nursery::new_from_stage(staged_holons, keyed_index);
        SpaceManager{
            space:None,  
            cache:HolonCache(Cache::new(99)), 
            nursery}
    }

    pub fn set_space_holon(&mut self, space: Holon) {
        self.space = Some(space);
    }

    pub fn create_space_holon(&mut self, context:&HolonsContext, holon:Holon)->  Result<CommitResponse,HolonError> {
        self.stage_new_holon(holon.clone())?;
        self.set_space_holon(holon);
        self.commit_stage(context)
    }

        //TODO: do we need this:

    /* pub fn get_description(&self) -> Result<MapString, HolonError> {
        let property_name = PropertyName(MapString("description".to_string()));

        match self.space.get_property_value(&property_name)? {
            PropertyValue::StringValue(name) => Ok(name),
            _ => Err(HolonError::InvalidType(format!(
                "Expected StringValue for '{}'",
                property_name.0
            ))),
        }
    } */
    pub fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        if let Some(ref holon) = self.space {
            holon.get_key()
        } else {
            Err(HolonError::NotAccessible("get_key".to_string(), "No space holon".to_string()))
        }
    }
    //TODO: do we need this:
/*     pub fn get_name(&self) -> Result<Option<MapString>, HolonError> {
        let property_name = PropertyName(MapString("name".to_string()));

        match self.space.get_property_value(&property_name)? {
            PropertyValue::StringValue(name) => Ok(name),
            _ => Err(HolonError::InvalidType(format!(
                "Expected StringValue for '{}'",
                property_name.0
            ))),
        }
    } */

    pub fn get_local_id(&self) -> Result<LocalId, HolonError> {
        if let Some(ref holon) = self.space {
            holon.get_local_id()
        } else {
            Err(HolonError::NotAccessible("get_local_id".to_string(), "No space holon".to_string()))
        }
    }

    fn holon_mut(&mut self) -> Option<&mut Holon> {
            self.space.as_mut() // Return a mutable reference to the inner `Holon`
        }
    
    pub fn into_holon(self) -> Holon {
            self.space.expect("No space holon found").clone()
        }

    pub fn with_description(&mut self, description: &MapString) -> Result<&mut Self, HolonError> {
        if let Some(holon) = self.holon_mut() {
            holon.with_property_value(
                PropertyName(MapString("description".to_string())),
                description.clone().into_base_value(),
            )?;
        } else {
            return Err(HolonError::NotAccessible("with_description".to_string(), "No space holon".to_string()));
        }
        Ok(self)
    }
    /// Sets the name property for the HolonSpace (and currently the "key" property)
    ///
    pub fn with_name(&mut self, name: &MapString) -> Result<&mut Self, HolonError> {
        if let Some(holon) = self.holon_mut() {
            holon.with_property_value(
                PropertyName(MapString("name".to_string())),
                name.clone().into_base_value(),
            )?
            // TODO: drop this once descriptor-based key support is implemented
            .with_property_value(
                PropertyName(MapString("key".to_string())),
                name.clone().into_base_value(),
            )?;
        } else {
            return Err(HolonError::NotAccessible("with_name".to_string(), "No space holon".to_string()));
        }
        Ok(self)
    }
    
    
    //SECTION Nursery adaptor functions
    
    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the staged holon
    /// to be retrieved by key
    pub fn stage_new_holon(&mut self, mut holon: Holon) -> Result<StagedReference, HolonError> {
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
        let holon_index = Nursery::add_new_holon(self.nursery.borrow_mut(), holon)?;
        Ok(StagedReference { holon_index })
    }


     /// This function finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon matching the
    /// specified key.
    /// NOTE: Only staged holons are searched and some holon types do not define unique keys
    /// This means that:
    ///    (1) even if this function returns `None` a holon with the specified key may exist in the DHT
    ///    (2) There might be some holons staged for update that you cannot find by key
    ///
    pub fn get_holon_by_key(&self, key: MapString) -> Option<Rc<RefCell<Holon>>> {
        Nursery::get_holon_by_key(&self.nursery, key)
    }

    
    /// Private helper function the encapsulates the logic for getting a mutable reference to a
    /// holon from a Staged
    // pub fn get_staged_reference(&self, index:StagedIndex)->Result<StagedReference, HolonError> {
    //     self.staged_holons.get(index.0 as usize)
    // }
    pub fn get_holon(&self, reference: &StagedReference) -> Result<Ref<Holon>, HolonError> {
        Nursery::get_holon_by_index(&self.nursery, reference.holon_index)
    }

    pub fn get_mut_holon(
        &self,
        staged_reference: &StagedReference,
    ) -> Result<RefMut<Holon>, HolonError> {
        Nursery::get_mut_holon_by_index(&self.nursery, staged_reference.holon_index)
    }

    pub fn get_mut_holon_by_index(
        &self,
        holon_index: StagedIndex,
    ) -> Result<RefMut<Holon>, HolonError> {
        Nursery::get_mut_holon_by_index(&self.nursery, holon_index)
    }

       /// This function converts a StagedIndex into a StagedReference
    /// Returns HolonError::IndexOutOfRange if index is out range for staged_holons vector
    /// Returns HolonError::NotAccessible if the staged holon is in an Abandoned state
    /// TODO: The latter is only reliable if staged_holons is made private
    pub fn to_staged_reference(
        &self,
        staged_index: StagedIndex,
    ) -> Result<StagedReference, HolonError> {
        if let Ok(staged_holon) = Nursery::get_holon_by_index(&self.nursery, staged_index) {//.staged_holons.get(staged_index) {
            let holon = staged_holon.borrow();
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

    pub fn get_stage(&self) -> Vec<Rc<RefCell<Holon>>> {
        Nursery::get_holon_stage(&self.nursery)
    }


    /// This method returns a mutable reference (Rc<RefCell>) to the Holon identified by holon_id.
    /// If holon_id is `Local`, it retrieves the holon from the local cache. If the holon is not
    /// already resident in the cache, this function first fetches the holon from the persistent
    /// store and inserts it into the cache before returning the reference to that holon.
    ///
    /// If the holon_id is `External`, this method currently returns a `NotImplemented` HolonError
    ///
    /// TODO: Enhance to support `External` HolonIds
    ///

    pub fn get_rc_holon(&mut self,holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError> {

        if let Some(holon) = self.cache.0.get(holon_id) {
            // Return a clone of the Rc<RefCell<Holon>> if found in the cache
            return Ok(Rc::clone(holon));
        }
        else {  // Holon not found in cache, fetch it
            let fetched_holon = match holon_id {
            Local(local_id) => Holon::get_holon_by_local_id(local_id)?,
            External(_) => {
                return Err(HolonError::NotImplemented(
                    "Fetch from external caches is not yet \
                implemented:"
                        .to_string(),
                ))
            }
            };
            //debug!("Holon with key {:?} fetched", fetched_holon.get_key());
            //debug!("Inserting fetched holon into cache for local_id: {:#?}", fetched_holon.get_local_id());
            self.cache.0.insert(holon_id.clone(), Rc::new(RefCell::new(fetched_holon)));

            // Return a new Rc<RefCell<Holon>> containing the fetched holon
            Ok(Rc::clone(self.cache.0.get(holon_id).expect("Holon should be present in the cache")))
        }
    }

/*     pub fn get_rc_holon(&self,holon_id: &HolonId) -> Result<Rc<RefCell<Holon>>, HolonError> {
 
        let cache = self.get_cache(holon_id)?;              Local(_) => Ok(Rc::clone(&self.local_cache)),


        // Attempt to borrow the cache immutably
        {
            let try_cache_borrow = cache.try_borrow().map_err(|e| {
                HolonError::FailedToBorrow(format!("Unable to borrow holon cache immutably: {}", e))
            })?;

            // Check if the holon is already in the cache
            debug!("Checking the cache for local_id: {:#?}", holon_id.local_id());
            if let Some(holon) = try_cache_borrow.0.get(holon_id) {
                // Return a clone of the Rc<RefCell<Holon>> if found in the cache
                return Ok(Rc::clone(holon));
            }
        }

        // Holon not found in cache, fetch it
        debug!("Holon not cached, fetching holon");

        let fetched_holon = match holon_id {
            Local(local_id) => HolonCacheManager::fetch_local_holon(local_id)?,
            External(_) => {
                return Err(HolonError::NotImplemented(
                    "Fetch from external caches is not yet \
                implemented:"
                        .to_string(),
                ))
            }
        };
        debug!("Holon with key {:?} fetched", fetched_holon.get_key());

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
    }*/


    pub fn commit_stage(&mut self,context:&HolonsContext) -> Result<CommitResponse, HolonError> {
        //todo borrow the commit service from the context
        let response = CommitService::commit(&self,context)?;
        if response.status == CommitRequestStatus::Complete {
            //TODO: Uncomment code and add saved holons to cache
            //for fetched_holon in response.saved_holons.clone() {
            //    if let Ok(local_id) = fetched_holon.get_local_id() {
            //        self.cache.0.insert(HolonId::from(local_id), Rc::new(RefCell::new(fetched_holon)));
            //    }
            // }
            Nursery::clear(&mut self.nursery);
            Ok(response)
        } else {
            Ok(response)
        }
    }

    //pub fn get_space_holon(&self) -> &Holon {
    //    &self.space
   // }

}
