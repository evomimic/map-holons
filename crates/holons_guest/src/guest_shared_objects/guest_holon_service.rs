use crate::guest_shared_objects::{
    commit_functions, create_local_path, get_holon_by_path, get_relationship_links,
};
use crate::persistence_layer::{create_holon_node, delete_holon_node, get_original_holon_node};
use hdk::prelude::*;

use holons_core::core_shared_objects::nursery_access_internal::NurseryAccessInternal;
use holons_core::core_shared_objects::{
    CommitResponse, Holon, HolonCollection, HolonError, HolonState, NurseryAccess, StagedRelationshipMap,
    RelationshipName,
};
use holons_core::reference_layer::{HolonServiceApi, HolonsContextBehavior};
use holons_core::HolonCollectionApi;
use holons_integrity::LinkTypes;
use shared_types_holon::{
    HolonId, LocalId, MapString, PropertyName, LOCAL_HOLON_SPACE_DESCRIPTION,
    LOCAL_HOLON_SPACE_NAME, LOCAL_HOLON_SPACE_PATH,
};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// #[hdk_entry_helper]
#[derive(Clone)]
pub struct GuestHolonService {
    /// Holds the internal nursery access after registration
    pub internal_nursery_access: Option<Arc<RefCell<dyn NurseryAccessInternal>>>,
}

impl GuestHolonService {
    pub fn new() -> Self {
        GuestHolonService {
            internal_nursery_access: None, // Initially, no privileged access
        }
    }
    /// ✅ HolonSpaceManager explicitly grants internal access at registration
    pub fn register_internal_access(&mut self, access: Arc<RefCell<dyn NurseryAccessInternal>>) {
        self.internal_nursery_access = Some(access);
    }

    /// Retrieves the stored internal access (set during registration)
    pub fn get_internal_nursery_access(
        &self,
    ) -> Result<Arc<RefCell<dyn NurseryAccessInternal>>, HolonError> {
        self.internal_nursery_access.clone().ok_or(HolonError::Misc(
            "GuestHolonService does not have internal nursery access.".to_string(),
        ))
    }
    /// Helper function to create a new Local Space Holon (including its Path) in the DHT
    ///
    /// # Arguments
    ///
    /// * `none`
    ///
    /// # Returns
    ///
    /// `Ok(Holon)` containing the newly created Local Space Holon if successful.
    /// `Err(HolonError)` if any errors occur during the process.
    fn create_local_space_holon(&self) -> Result<Holon, HolonError> {
        // Define the name and description for the local space holon
        let name: MapString = MapString(LOCAL_HOLON_SPACE_NAME.to_string());
        let description: MapString = MapString(LOCAL_HOLON_SPACE_DESCRIPTION.to_string());

        // Create a new Holon and set its name and description
        let mut space_holon = Holon::new();
        space_holon
            .with_property_value(
                PropertyName(MapString("name".to_string())),
                name.clone().into_base_value(),
            )?
            .with_property_value(
                PropertyName(MapString("key".to_string())),
                name.clone().into_base_value(),
            )?
            .with_property_value(
                PropertyName(MapString("description".to_string())),
                description.into_base_value(),
            )?;

        // Try to create the holon node in the DHT
        let result = create_holon_node(space_holon.clone().into_node());

        match result {
            Ok(record) => {
                // If successful, update the holon state and saved node
                space_holon.state = HolonState::Saved;
                space_holon.saved_node = Some(record);
            }
            Err(error) => {
                // If there’s an error, return it as a HolonError
                return Err(HolonError::from(error));
            }
        }

        // Retrieve the local ID for the holon
        let local_id = space_holon.get_local_id()?;

        // Log the creation of the LocalHolonSpace
        info!("Created LocalHolonSpace with id {:#?}", local_id);

        // Try to create the local path for the holon
        create_local_path(
            local_id,
            LOCAL_HOLON_SPACE_PATH.to_string(),
            LinkTypes::LocalHolonSpace,
        )?;

        // Return the created holon
        Ok(space_holon)
    }

    /// "Guard" function that confirms the HolonId is a LocalId and, if not, returns an
    /// InvalidParameter error.
    fn ensure_id_is_local(id: &HolonId) -> Result<LocalId, HolonError> {
        match id {
            HolonId::Local(local_id) => Ok(local_id.clone()),
            HolonId::External(_) => {
                Err(HolonError::InvalidParameter("Expected LocalId, found ExternalId.".to_string()))
            }
        }
    }
    /// Ensure that a Local Space Holon exists in the DHT and, if not, creates one. This method
    /// is intended to be implemented by guest
    ///
    /// This function attempts to fetch the SpaceHolon from the persistent store. If that fails,
    /// it creates one.
    ///
    /// # Arguments
    ///
    /// * *none*
    ///
    /// # Returns
    ///
    /// `Ok(Holon)` the Local Space Holon reference if successful.
    /// `Err(HolonError)` if any errors occur during the process.
    pub fn ensure_local_holon_space(&self) -> Result<Holon, HolonError> {
        let space_holon_result =
            get_holon_by_path(LOCAL_HOLON_SPACE_PATH.to_string(), LinkTypes::LocalHolonSpace)?;
        match space_holon_result {
            Some(holon) => Ok(holon),
            None => {
                info!("Local Space Holon not found in storage, creating a new one.");
                self.create_local_space_holon()
            }
        }
    }

    // fn get_internal_nursery_access(
    //     &self,
    //     context: &dyn HolonsContextBehavior,
    // ) -> Result<Arc<RefCell<dyn NurseryAccessInternal>>, HolonError> {
    //     // Retrieve the registered internal access from the space manager
    //     let space_manager = context.get_space_manager();
    //     match space_manager.get_registered_internal_nursery_access() {
    //         Some(internal_access) => Ok(internal_access),
    //         None => Err(HolonError::Misc(
    //             "GuestHolonService does not have internal nursery access.".to_string(),
    //         )),
    //     }
    // }
    pub fn get_nursery_access(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Arc<RefCell<dyn NurseryAccess>> {
        // Retrieve the space manager from the context
        let space_manager = context.get_space_manager();

        space_manager.get_nursery_access()
    }
}

impl HolonServiceApi for GuestHolonService {
    fn commit(&self, context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError> {
        // Get internal nursery access
        let internal_nursery = self.get_internal_nursery_access()?;

        // ✅ Step 1: Borrow immutably, immediately clone the Vec, then drop the borrow
        let staged_holons = {
            let nursery_read = internal_nursery.borrow();
            let cloned_holons = nursery_read.get_holons_to_commit().clone(); // ✅ Clone while borrow is active
            cloned_holons // ✅ Borrow ends here
        }; // ✅ `nursery_read` is dropped immediately after this block

        // ✅ Step 2: Commit the staged holons
        let commit_response = commit_functions::commit(context, &staged_holons)?;

        // ✅ Step 3: Borrow mutably to clear the stage
        internal_nursery.borrow_mut().clear_stage(); // ✅ Safe, no borrow conflict

        // ✅ Step 4: Return the commit response
        Ok(commit_response)
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

    /// gets a specific HolonNode from the local persistent store based on the original ActionHash,
    /// then "inflates" the HolonNode into a Holon and returns it
    fn fetch_holon(&self, holon_id: &HolonId) -> Result<Holon, HolonError> {
        let local_id = Self::ensure_id_is_local(holon_id)?;

        let holon_node_record = get_original_holon_node(local_id.0.clone())?; // Retrieve the holon node
        if let Some(node) = holon_node_record {
            let holon = Holon::try_from_node(node)?;
            Ok(holon)
        } else {
            // No holon_node fetched for the specified holon_id
            Err(HolonError::HolonNotFound(local_id.0.to_string()))
        }
    }

    fn fetch_related_holons(
        &self,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        let local_id = Self::ensure_id_is_local(source_id)?;

        let mut collection = HolonCollection::new_existing();

        // fetch the smartlinks for this relationship (if any)
        let smartlinks = get_relationship_links(local_id.0, relationship_name)?;
        debug!("Got {:?} smartlinks: {:#?}", smartlinks.len(), smartlinks);

        for smartlink in smartlinks {
            let holon_reference = smartlink.to_holon_reference();
            collection.add_reference_with_key(smartlink.get_key().as_ref(), &holon_reference)?;
        }
        Ok(collection)
    }

    fn fetch_all_populated_relationships(
        &self,
        _source_id: HolonId,
    ) -> Result<StagedRelationshipMap, HolonError> {
        todo!()
    }
}

// ✅ Manually implement Debug (exclude internal_nursery_access)
impl fmt::Debug for GuestHolonService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuestHolonService")
            .field("internal_nursery_access", &"<hidden>") // ✅ Hide the trait object
            .finish()
    }
}
