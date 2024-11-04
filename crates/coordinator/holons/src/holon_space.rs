use std::borrow::BorrowMut;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::rc::Rc;
use quick_cache::unsync::Cache;
use hdi::prelude::{Deserialize, Path, Serialize};

use holons_integrity::LinkTypes;
use shared_types_holon::{HolonId, LocalId, MapString, PropertyName, PropertyValue};
use shared_types_holon::HolonId::{External, Local};

use crate::commit_service::{commit_holon, commit_rc_holons, CommitRequestStatus, CommitResponse};
use crate::context::HolonsContext;
use crate::holon::{Holon, HolonState};
use crate::holon_collection::CollectionState;
use crate::holon_error::HolonError;
use crate::holon_node::{
    create_path_to_holon_node, get_holon_node_by_path, CreatePathInput, GetPathInput,
};
use crate::staged_reference::StagedReference;

pub type StagedIndex = usize;

#[derive(Debug, Clone)]
pub struct HolonCache(Cache<HolonId, Rc<RefCell<Holon>>>);

#[derive(Debug, Clone)]
pub struct HolonSpace {
    space: Holon,
    cache: HolonCache, //already committed holons
    staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
    keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}

impl HolonSpace {
    pub fn new(holon: Holon) -> HolonSpace {
        // Initialize local cache
        let cache = Cache::new(99);
  
        // Wrap local_cache in a Rc<RefCell<_>>
        let local_cache = HolonCache(cache);
        HolonSpace{space:holon,cache:local_cache, staged_holons: Vec::new(), keyed_index: Default::default()}
    }

    //self reflective accessor and mutator functions
    pub fn get_description(&self) -> Result<MapString, HolonError> {
        let property_name = PropertyName(MapString("description".to_string()));

        match self.space.get_property_value(&property_name)? {
            PropertyValue::StringValue(name) => Ok(name),
            _ => Err(HolonError::InvalidType(format!(
                "Expected StringValue for '{}'",
                property_name.0
            ))),
        }
    }
    pub fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        self.space.get_key()
    }
    pub fn get_name(&self) -> Result<MapString, HolonError> {
        let property_name = PropertyName(MapString("name".to_string()));

        match self.space.get_property_value(&property_name)? {
            PropertyValue::StringValue(name) => Ok(name),
            _ => Err(HolonError::InvalidType(format!(
                "Expected StringValue for '{}'",
                property_name.0
            ))),
        }
    }

    pub fn get_local_id(&self) -> Result<LocalId, HolonError> {
        self.space.get_local_id()
    }

    fn holon_mut(&mut self) -> &mut Holon {
        &mut self.space // Return a mutable reference to the inner `Holon`
    }
    pub fn into_holon(self) -> Holon {
        self.space.clone()
    }

    pub fn with_description(&mut self, description: &MapString) -> Result<&mut Self, HolonError> {
        self.holon_mut().with_property_value(
            PropertyName(MapString("description".to_string())),
            description.clone().into_base_value(),
        )?;
        Ok(self)
    }
    /// Sets the name property for the HolonSpace (and currently the "key" property)
    ///
    pub fn with_name(&mut self, name: &MapString) -> Result<&mut Self, HolonError> {
        self.holon_mut()
            .with_property_value(
                PropertyName(MapString("name".to_string())),
                name.clone().into_base_value(),
            )?
            // TODO: drop this once descriptor-based key support is implemented
            .with_property_value(
                PropertyName(MapString("key".to_string())),
                name.clone().into_base_value(),
            )?;
        Ok(self)
    }

    pub fn clear_staged_objects(&mut self) {
        self.staged_holons.clear();
        self.keyed_index.clear();
    }



     /// This function finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon matching the
    /// specified key.
    /// NOTE: Only staged holons are searched and some holon types do not define unique keys
    /// This means that:
    ///    (1) even if this function returns `None` a holon with the specified key may exist in the DHT
    ///    (2) There might be some holons staged for update that you cannot find by key
    ///
    pub fn get_holon_by_key(&self, key: MapString) -> Option<Rc<RefCell<Holon>>> {
        if let Some(&index) = self.keyed_index.get(&key) {
            Some(Rc::clone(&self.staged_holons[index]))
        } else {
            None
        }
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

        // debug!("Checking the cache for local_id: {:#?}", holon_id.local_id());
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


    /// Private helper function the encapsulates the logic for getting a mutable reference to a
    /// holon from a Staged
    // pub fn get_staged_reference(&self, index:StagedIndex)->Result<StagedReference, HolonError> {
    //     self.staged_holons.get(index.0 as usize)
    // }
    pub fn get_holon(&self, reference: &StagedReference) -> Result<Ref<Holon>, HolonError> {
        let holons = &self.staged_holons;
        let holon_ref = holons
            .get(reference.holon_index)
            .ok_or_else(|| HolonError::IndexOutOfRange(reference.holon_index.to_string()))?;

        match holon_ref.try_borrow() {
            Ok(holon) => Ok(holon),
            Err(_) => Err(HolonError::FailedToBorrow(
                "Holon Reference from staged_holons vector".to_string(),
            )),
        }
    }

    /// Private helper function that encapsulates the logic for getting a mutable reference to a
    /// holon from a StagedReference
    fn get_mut_holon_internal(
        &self,
        holon_index: Option<StagedIndex>,
    ) -> Result<RefMut<Holon>, HolonError> {
        if let Some(index) = holon_index {
            if let Some(holon) = self.staged_holons.get(index) {
                return if let Ok(holon_refcell) = holon.try_borrow_mut() {
                    Ok(holon_refcell)
                } else {
                    Err(HolonError::FailedToBorrow("for StagedReference".to_string()))
                };
            }
        }
        Err(HolonError::InvalidHolonReference("Invalid holon index".to_string()))
    }

    pub fn get_mut_holon(
        &self,
        staged_reference: &StagedReference,
    ) -> Result<RefMut<Holon>, HolonError> {
        self.get_mut_holon_internal(Some(staged_reference.holon_index))
    }

    pub fn get_mut_holon_by_index(
        &self,
        holon_index: StagedIndex,
    ) -> Result<RefMut<Holon>, HolonError> {
        self.get_mut_holon_internal(Some(holon_index))
    }

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the CommitManager's keyed_index to allow the staged holon
    /// to be retrieved by key

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the CommitManager's keyed_index to allow the staged holon
    /// to be retrieved by key
    pub fn stage_new_holon(&mut self, holon: Holon) -> Result<StagedReference, HolonError> {
        let mut cloned_holon = holon.clone();
        for (_relationship_name, collection) in cloned_holon.relationship_map.0.iter_mut() {
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

        let rc_holon = Rc::new(RefCell::new(cloned_holon));
        self.staged_holons.push(Rc::clone(&rc_holon));
        //trace!("Added to StagingArea, Holon: {:#?}", rc_holon);
        let holon_index = self.staged_holons.len() - 1;
        let holon_key: Option<MapString> = holon.get_key()?;
        if let Some(key) = holon_key.clone() {
            self.keyed_index.insert(key.clone(), holon_index);
        }
       // trace!("Success! Holon staged, with key: {:?}, at index: {:?}", holon_key, holon_index);

        Ok(StagedReference { holon_index })
    }

    /// This function converts a StagedIndex into a StagedReference
    /// Returns HolonError::IndexOutOfRange if index is out range for staged_holons vector
    /// Returns HolonError::NotAccessible if the staged holon is in an Abandoned state
    /// TODO: The latter is only reliable if staged_holons is made private
    pub fn to_staged_reference(
        &self,
        staged_index: StagedIndex,
    ) -> Result<StagedReference, HolonError> {
        if let Some(staged_holon) = self.staged_holons.get(staged_index) {
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

    pub fn commit_staged_holons(mut self) -> CommitResponse {
        let response = commit_rc_holons(&self.staged_holons);
        if response.status == CommitRequestStatus::Complete {
            //add saved holoons to cache
            for fetched_holon in response.saved_holons.clone() {
                if let Ok(local_id) = fetched_holon.get_local_id() {
                    self.cache.0.insert(HolonId::from(local_id), Rc::new(RefCell::new(fetched_holon)));
                }
            }
            self.clear_staged_objects();
            response
        } else {
            response
        }
    }

    pub fn commit_space(&mut self) -> CommitResponse {
        commit_holon(self.space.borrow_mut())
    }
}
