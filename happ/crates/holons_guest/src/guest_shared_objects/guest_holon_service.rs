use std::any::Any;
use std::collections::HashMap;
use std::{
    fmt,
    sync::{Arc, RwLock},
};

use hdk::prelude::*;
use holons_core::core_shared_objects::SavedHolon;
use holons_core::reference_layer::{ReadableHolon, TransientReference};
use holons_core::RelationshipMap;
use holons_guest_integrity::type_conversions::{
    holon_error_from_wasm_error, try_action_hash_from_local_id,
};
use holons_guest_integrity::{
    HolonNode, LOCAL_HOLON_SPACE_DESCRIPTION, LOCAL_HOLON_SPACE_NAME, LOCAL_HOLON_SPACE_PATH,
};

use super::{fetch_links_to_all_holons, get_all_relationship_links};
use crate::guest_shared_objects::{commit_functions, get_relationship_links};
use crate::persistence_layer::{create_holon_node, delete_holon_node, get_original_holon_node};
use crate::{create_local_path, get_holon_by_path, try_from_record};
use base_types::MapString;
use core_types::{HolonError, HolonId};
use holons_core::core_shared_objects::transactions::TransactionContextHandle;
use holons_core::{
    core_shared_objects::{
        nursery_access_internal::NurseryAccessInternal, transactions::TransactionContext, Holon,
        HolonCollection,
    },
    reference_layer::{
        HolonCollectionApi, HolonReference, HolonServiceApi, SmartReference, WritableHolon,
    },
};
use holons_integrity::LinkTypes;
use holons_loader::HolonLoaderController;
use integrity_core_types::{LocalId, PropertyMap, PropertyName, RelationshipName};

pub struct GuestHolonService {
    /// Holds the internal nursery access after registration
    pub internal_nursery_access: RwLock<Option<Arc<RwLock<dyn NurseryAccessInternal>>>>,
}

impl GuestHolonService {
    pub fn new() -> Self {
        GuestHolonService {
            internal_nursery_access: RwLock::new(None), // Initially, no privileged access
        }
    }
    /// ✅ HolonSpaceManager explicitly grants internal access at registration
    pub fn register_internal_access(&self, access: Arc<RwLock<dyn NurseryAccessInternal>>) {
        let mut guard = self
            .internal_nursery_access
            .write()
            .expect("Failed to acquire write lock on internal nursery access");
        *guard = Some(access);
    }

    /// Retrieves the stored internal access (set during registration)
    pub fn get_internal_nursery_access(
        &self,
    ) -> Result<Arc<RwLock<dyn NurseryAccessInternal>>, HolonError> {
        let guard = self.internal_nursery_access.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on internal nursery access: {}",
                e
            ))
        })?;
        guard.clone().ok_or(HolonError::Misc(
            "GuestHolonService does not have internal nursery access.".to_string(),
        ))
    }

    fn create_local_space_holon(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<SavedHolon, HolonError> {
        // Define the name and description for the local space holon
        let name: MapString = MapString(LOCAL_HOLON_SPACE_NAME.to_string());
        let description: MapString = MapString(LOCAL_HOLON_SPACE_DESCRIPTION.to_string());

        // Obtain the externally visible TransientHolonBehavior service for creating a new holon.
        let transient_behavior_service = context.get_transient_behavior_service();

        // Create new (empty) TransientHolon
        let mut space_holon_reference = transient_behavior_service.create_empty(name.clone())?;
        space_holon_reference
            .with_property_value(
                PropertyName(MapString("name".to_string())),
                name.clone().into_base_value(),
            )?
            .with_property_value(
                PropertyName(MapString("description".to_string())),
                description.into_base_value(),
            )?;
        let space_holon_node = space_holon_reference.into_model()?;

        // Try to create the holon node in the DHT
        let holon_record = create_holon_node(HolonNode::from(space_holon_node.clone()))
            .map_err(|e| holon_error_from_wasm_error(e))?;
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
    pub fn ensure_local_holon_space(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonReference, HolonError> {
        let space_holon_result =
            get_holon_by_path(LOCAL_HOLON_SPACE_PATH.to_string(), LinkTypes::LocalHolonSpace)?;

        let holon = match space_holon_result {
            Some(holon) => holon,
            None => {
                info!("Local Space Holon not found in storage, creating a new one.");
                self.create_local_space_holon(context)?
            }
        };

        let local_id = holon.get_local_id()?;

        // Reacquire Arc<TransactionContext> to mint a tx-bound SmartReference.
        let context_arc = context
            .space_manager()
            .get_transaction_manager()
            .get_transaction(&context.tx_id())?
            .ok_or_else(|| HolonError::ServiceNotAvailable("TransactionContext".into()))?;

        let handle = TransactionContextHandle::new(context_arc);

        Ok(HolonReference::Smart(SmartReference::new_from_id(handle, HolonId::Local(local_id))))
    }

    fn mint_smart_reference_from_pointer(
        &self,
        context: &Arc<TransactionContext>,
        holon_id: HolonId,
        smart_property_values: Option<PropertyMap>,
    ) -> Result<HolonReference, HolonError> {
        let handle = TransactionContextHandle::new(Arc::clone(context));

        let smart = match smart_property_values {
            Some(props) => SmartReference::new_with_properties(handle, holon_id, props),
            None => SmartReference::new_from_id(handle, holon_id),
        };

        Ok(HolonReference::Smart(smart))
    }
}

impl HolonServiceApi for GuestHolonService {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn commit_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<TransientReference, HolonError> {
        // Get internal nursery access
        let internal_nursery = self.get_internal_nursery_access()?;

        // Step 1: Borrow the nursery immutably and clone its HolonPool reference
        let staged_references = {
            let nursery_read = internal_nursery.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on internal NurseryAccess: {}",
                    e
                ))
            })?;
            nursery_read.get_staged_references()?
        }; // `nursery_read` is dropped immediately after this block

        // Step 2: Commit the staged holons
        let commit_response = commit_functions::commit(context, &staged_references)?;

        // Step 3: Borrow mutably to clear the stage
        let _ = internal_nursery.write().unwrap().clear_stage(); // Safe, no borrow conflict

        // Step 4: Return the commit response
        Ok(commit_response)
    }

    fn delete_holon_internal(&self, local_id: &LocalId) -> Result<(), HolonError> {
        let record = get(try_action_hash_from_local_id(&local_id)?, GetOptions::default())
            .map_err(|e| holon_error_from_wasm_error(e))?
            .ok_or_else(|| HolonError::HolonNotFound(format!("at id: {:?}", local_id.0)))?;
        let _holon = try_from_record(record)?;
        // holon.is_deletable()?;
        delete_holon_node(try_action_hash_from_local_id(&local_id)?)
            .map(|_| ()) // Convert ActionHash to ()
            .map_err(|e| holon_error_from_wasm_error(e))
    }

    fn fetch_all_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        if !source_id.is_local() {
            return Err(HolonError::InvalidHolonReference("Source id must be Local".to_string()));
        }

        let mut relationship_map: HashMap<RelationshipName, Arc<RwLock<HolonCollection>>> =
            HashMap::new();

        let mut reference_map: HashMap<RelationshipName, Vec<HolonReference>> = HashMap::new();

        let smartlinks = get_all_relationship_links(source_id.local_id())?;
        debug!("Retrieved {:?} smartlinks", smartlinks.len());

        for smartlink in smartlinks {
            let (holon_id, smart_props) = smartlink.to_pointer();
            let reference =
                self.mint_smart_reference_from_pointer(context, holon_id, smart_props)?;

            // The following:
            // 1) adds an entry for relationship name if not already present (via `entry` API)
            // 2) adds a value (Vec<HolonReference>) for the entry, if not already present (`.or_insert_with`)
            // 3) pushes the new HolonReference into the vector -- without having to clone the vector

            reference_map
                .entry(smartlink.relationship_name)
                .or_insert_with(Vec::new)
                .push(reference);
        }

        for (map_name, holon_references) in reference_map {
            let mut collection = HolonCollection::new_existing();
            for reference in holon_references {
                let key = reference.key()?.ok_or_else(|| {
                    HolonError::Misc(
                        "Expected Smartlink to have a key, didn't get one.".to_string(),
                    )
                })?; // At least for now, all SmartLinks should be encoded with a key
                collection.add_reference_with_key(Some(&key), &reference)?;
            }
            relationship_map.insert(map_name, Arc::new(RwLock::new(collection)));
        }

        Ok(RelationshipMap::new(relationship_map))
    }

    /// gets a specific HolonNode from the local persistent store based on the original ActionHash,
    /// then "inflates" the HolonNode into a Holon and returns it
    fn fetch_holon_internal(&self, holon_id: &HolonId) -> Result<Holon, HolonError> {
        let local_id = Self::ensure_id_is_local(holon_id)?;

        // Retrieve the exact HolonNode for the specific ActionHash.
        // DISCLAIMER: The name of this scaffolded function is misleading... it does not 'walk the tree' to get the original record.
        // keeping the terminology per policy not to change scaffolded code.
        let holon_node_record = get_original_holon_node(try_action_hash_from_local_id(&local_id)?)
            .map_err(|e| holon_error_from_wasm_error(e))?;
        if let Some(record) = holon_node_record {
            let holon = try_from_record(record)?;
            Ok(Holon::Saved(holon))
        } else {
            // No holon_node fetched for the specified holon_id
            Err(HolonError::HolonNotFound(local_id.to_string()))
        }
    }

    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        let local_id = Self::ensure_id_is_local(source_id)?;

        let mut collection = HolonCollection::new_existing();

        // fetch the smartlinks for this relationship (if any)
        let smartlinks =
            get_relationship_links(try_action_hash_from_local_id(&local_id)?, relationship_name)?;
        debug!("Got {:?} smartlinks: {:#?}", smartlinks.len(), smartlinks);

        for smartlink in smartlinks {
            let (holon_id, smart_props) = smartlink.to_pointer();
            let holon_reference =
                self.mint_smart_reference_from_pointer(context, holon_id, smart_props)?;
            collection.add_reference_with_key(smartlink.key()?.as_ref(), &holon_reference)?;
        }
        Ok(collection)
    }

    fn get_all_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        let mut collection = HolonCollection::new_existing();
        let holon_ids = fetch_links_to_all_holons()?;
        let mut holon_references = Vec::new();
        for id in holon_ids {
            holon_references.push(self.mint_smart_reference_from_pointer(context, id, None)?);
        }
        collection.add_references(holon_references)?;

        Ok(collection)
    }

    /// Execute a Holon import from a `HolonLoadSet`.
    /// Delegates to the `HolonLoaderController` and returns a transient `HolonLoadResponse`.
    fn load_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        set: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        // Construct controller and delegate to load_set()
        let mut controller = HolonLoaderController::new();
        controller.load_set(context, set)
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
