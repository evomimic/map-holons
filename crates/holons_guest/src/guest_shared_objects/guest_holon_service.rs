use std::{cell::RefCell, collections::BTreeMap, fmt, rc::Rc, sync::Arc};

use hdk::prelude::*;

use super::{fetch_links_to_all_holons, get_all_relationship_links};
use crate::guest_shared_objects::{
    commit_functions, create_local_path, get_holon_by_path, get_relationship_links,
};
use crate::persistence_layer::{create_holon_node, delete_holon_node, get_original_holon_node};
use crate::try_from_record;
use base_types::{BaseValue, MapString};
use core_types::{HolonError, HolonId};
use holons_core::{
    core_shared_objects::{
        holon::state::AccessType, nursery_access_internal::NurseryAccessInternal, CommitResponse,
        Holon, HolonBehavior, HolonCollection, NurseryAccess, RelationshipName, TransientHolon,
        TransientRelationshipMap,
    },
    reference_layer::{
        HolonCollectionApi, HolonReference, HolonServiceApi, HolonsContextBehavior, ReadableHolon,
        SmartReference, StagedReference,
    },
    WriteableHolon,
};
use holons_integrity::LinkTypes;
use integrity_core_types::{
    LocalId, PropertyName, WasmErrorWrapper, LOCAL_HOLON_SPACE_DESCRIPTION, LOCAL_HOLON_SPACE_NAME,
    LOCAL_HOLON_SPACE_PATH,
};

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
    // /// A private helper method for populating a StagedRelationshipMap for a newly staged Holon by cloning all existing relationships from a persisted Holon.
    // ///
    // /// Populates a full StagedRelationshipMap by retrieving all SmartLinks for which this holon is the
    // /// source. The map returned will ONLY contain entries for relationships that have at least
    // /// one related holon (i.e., none of the holon collections returned via the result map will have
    // /// zero members).
    // fn clone_existing_relationships_into_staged_map(
    //     &self,
    //     context: &dyn HolonsContextBehavior,
    //     original_holon: HolonId,
    // ) -> Result<StagedRelationshipMap, HolonError> {
    //     debug!("Loading all relationships...");
    //     let mut relationship_map: BTreeMap<RelationshipName, Rc<RefCell<HolonCollection>>> =
    //         BTreeMap::new();

    //     let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();

    //     let smartlinks = get_all_relationship_links(original_holon.local_id())
    //         .map_err(|e| HolonError::InvalidParameter(e.to_string()))?;
    //     debug!("Retrieved {:?} smartlinks", smartlinks.len());

    //     for smartlink in smartlinks {
    //         let reference = smartlink.to_holon_reference();

    //         // The following:
    //         // 1) adds an entry for relationship name if not already present (via `entry` API)
    //         // 2) adds a value (Vec<HolonReference>) for the entry, if not already present (`.or_insert_with`)
    //         // 3) pushes the new HolonReference into the vector -- without having to clone the vector

    //         reference_map
    //             .entry(smartlink.relationship_name)
    //             .or_insert_with(Vec::new)
    //             .push(reference);
    //     }

    //     // Populate relationship_map

    //     for (map_name, holons) in reference_map {
    //         let mut collection = HolonCollection::new_existing();
    //         collection.add_references(context, holons)?;
    //         relationship_map.insert(map_name, Rc::new(RefCell::new(collection)));
    //     }

    //     Ok(StagedRelationshipMap { map: relationship_map })
    // }

    /// A private helper method for populating a TransientRelationshipMap for a TransientHolon by cloning all existing relationships from a persisted Holon.
    ///
    /// Populates a full TransientRelationshipMap by retrieving all SmartLinks for which this holon is the
    /// source. The map returned will ONLY contain entries for relationships that have at least
    /// one related holon (i.e., none of the holon collections returned via the result map will have
    /// zero members).
    fn clone_existing_relationships_into_transient_map(
        &self,
        context: &dyn HolonsContextBehavior,
        original_holon: HolonId,
    ) -> Result<TransientRelationshipMap, HolonError> {
        debug!("Loading all relationships...");
        let mut relationship_map: BTreeMap<RelationshipName, Rc<RefCell<HolonCollection>>> =
            BTreeMap::new();

        let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();

        let smartlinks = get_all_relationship_links(original_holon.local_id())
            .map_err(|e| HolonError::InvalidParameter(e.to_string()))?;
        debug!("Retrieved {:?} smartlinks", smartlinks.len());

        for smartlink in smartlinks {
            let reference = smartlink.to_holon_reference();

            // The following:
            // 1) adds an entry for relationship name if not already present (via `entry` API)
            // 2) adds a value (Vec<HolonReference>) for the entry, if not already present (`.or_insert_with`)
            // 3) pushes the new HolonReference into the vector -- without having to clone the vector

            reference_map
                .entry(smartlink.relationship_name)
                .or_insert_with(Vec::new)
                .push(reference);
        }

        // Populate relationship_map

        for (map_name, holons) in reference_map {
            let mut collection = HolonCollection::new_transient();
            collection.add_references(context, holons)?;
            relationship_map.insert(map_name, Rc::new(RefCell::new(collection)));
        }

        Ok(TransientRelationshipMap::new(relationship_map))
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
        let mut space_holon = TransientHolon::new();
        space_holon
            .with_property_value(
                PropertyName(MapString("name".to_string())),
                Some(name.clone().into_base_value()),
            )?
            .with_property_value(
                PropertyName(MapString("key".to_string())),
                Some(name.clone().into_base_value()),
            )?
            .with_property_value(
                PropertyName(MapString("description".to_string())),
                Some(description.into_base_value()),
            )?;
        let space_holon_node = space_holon.clone().into_node();

        // Try to create the holon node in the DHT
        let holon_record = create_holon_node(space_holon_node.clone())
            .map_err(|e| HolonError::from_wasm_error(WasmErrorWrapper(e)))?;
        let saved_holon = try_from_record(holon_record)?;

        // Retrieve the local ID for the holon
        let local_id = saved_holon.get_local_id()?;

        // Log the creation of the LocalHolonSpace
        info!("Created LocalHolonSpace with id {:#?}", local_id);

        // Try to create the local path for the holon
        create_local_path(
            local_id,
            LOCAL_HOLON_SPACE_PATH.to_string(),
            LinkTypes::LocalHolonSpace,
        )?;

        // Return the created holon
        Ok(saved_holon)
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
    /// Ensures that a Local Space Holon exists in the DHT. If not, it creates one.
    ///
    /// This function attempts to fetch the SpaceHolon from persistent storage.
    /// If previously saved, return a HolonReference to it.
    /// Otherwise, create (and persist) it and return a HolonReference to it.
    ///
    /// # Returns
    ///
    /// * `Ok(HolonReference::Smart)` – The Local Space Holon reference if successful.
    /// * `Err(HolonError)` – If any errors occur during retrieval or creation.
    pub fn ensure_local_holon_space(&self) -> Result<HolonReference, HolonError> {
        let space_holon_result =
            get_holon_by_path(LOCAL_HOLON_SPACE_PATH.to_string(), LinkTypes::LocalHolonSpace)?;

        let holon = match space_holon_result {
            Some(holon) => holon,
            None => {
                info!("Local Space Holon not found in storage, creating a new one.");
                self.create_local_space_holon()?
            }
        };

        holon
            .get_local_id()
            .map(|id| HolonReference::Smart(SmartReference::new_from_id(HolonId::Local(id))))
            .map_err(|e| {
                error!("Failed to retrieve local holon ID: {:?}", e);
                e
            })
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

        // Step 1: Borrow immutably, immediately clone the Vec, then drop the borrow
        let staged_holons = {
            let nursery_read = internal_nursery.borrow();
            let cloned_holons = nursery_read.get_holons_to_commit().clone(); // Clone while borrow is active
            cloned_holons // Borrow ends here
        }; // `nursery_read` is dropped immediately after this block

        // Step 2: Commit the staged holons
        let commit_response = commit_functions::commit(context, &staged_holons)?;

        // Step 3: Borrow mutably to clear the stage
        internal_nursery.borrow_mut().clear_stage(); // Safe, no borrow conflict

        // Step 4: Return the commit response
        Ok(commit_response)
    }

    fn delete_holon(&self, local_id: &LocalId) -> Result<(), HolonError> {
        let record = get(local_id.0.clone(), GetOptions::default())
            .map_err(|e| HolonError::from_wasm_error(WasmErrorWrapper(e)))?
            .ok_or_else(|| HolonError::HolonNotFound(format!("at id: {:?}", local_id.0)))?;
        let mut holon = try_from_record(record)?;
        // holon.is_deletable()?;
        delete_holon_node(local_id.0.clone())
            .map(|_| ()) // Convert ActionHash to ()
            .map_err(|e| HolonError::from_wasm_error(WasmErrorWrapper(e)))
    }

    /// gets a specific HolonNode from the local persistent store based on the original ActionHash,
    /// then "inflates" the HolonNode into a Holon and returns it
    fn fetch_holon(&self, holon_id: &HolonId) -> Result<Holon, HolonError> {
        let local_id = Self::ensure_id_is_local(holon_id)?;

        // Retrieve the exact HolonNode for the specific ActionHash.
        // DISCLAIMER: The name of this scaffolded function is misleading... it does not 'walk the tree' to get the original record.
        // keeping the terminology per policy not to change scaffolded code.
        let holon_node_record = get_original_holon_node(local_id.0.clone())
            .map_err(|e| HolonError::from_wasm_error(WasmErrorWrapper(e)))?;
        if let Some(record) = holon_node_record {
            let holon = try_from_record(record)?;
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
            collection.add_reference_with_key(smartlink.get_key()?.as_ref(), &holon_reference)?;
        }
        Ok(collection)
    }

    fn get_all_holons(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCollection, HolonError> {
        let mut collection = HolonCollection::new_existing();
        let holon_ids = fetch_links_to_all_holons()?;
        let mut holon_references = Vec::new();
        for id in holon_ids {
            holon_references.push(HolonReference::from_id(id));
        }
        collection.add_references(context, holon_references)?;

        Ok(collection)
    }

    /// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining
    /// lineage to the Holon its cloned from.
    fn stage_new_from_clone(
        &self,
        context: &dyn HolonsContextBehavior,
        original_holon: HolonReference,
        new_key: MapString,
    ) -> Result<StagedReference, HolonError> {
        original_holon.is_accessible(context, AccessType::Clone)?;

        let mut cloned_holon = original_holon.clone_holon(context)?;

        // update key
        cloned_holon.with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(new_key)),
        )?;

        // Reset the OriginalId to None
        cloned_holon.update_original_id(None)?;

        match original_holon {
            HolonReference::Transient(_) => {}
            HolonReference::Staged(_) => {}
            HolonReference::Smart(_) => cloned_holon.update_relationship_map(
                self.clone_existing_relationships_into_transient_map(
                    context,
                    original_holon.get_holon_id(context)?,
                )?,
            )?,
        }

        let cloned_staged_reference =
            self.get_internal_nursery_access()?.borrow().stage_new_holon(cloned_holon)?;

        // Reset the PREDECESSOR to None
        cloned_staged_reference.with_predecessor(context, None)?;

        Ok(cloned_staged_reference)
    }

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the staged holon
    /// to be retrieved by key.
    fn stage_new_version(
        &self,
        context: &dyn HolonsContextBehavior,
        original_holon: SmartReference,
    ) -> Result<StagedReference, HolonError> {
        original_holon.is_accessible(context, AccessType::Clone)?;

        let mut cloned_holon = original_holon.clone_holon(context)?;

        cloned_holon.update_relationship_map(
            self.clone_existing_relationships_into_transient_map(
                context,
                original_holon.get_holon_id(context)?,
            )?,
        )?;

        let cloned_staged_reference =
            self.get_internal_nursery_access()?.borrow().stage_new_holon(cloned_holon)?;

        // Reset the PREDECESSOR to the original Holon being cloned from.
        cloned_staged_reference
            .with_predecessor(context, Some(HolonReference::Smart(original_holon)))?;

        Ok(cloned_staged_reference)
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
