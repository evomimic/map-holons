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
    LOCAL_HOLON_SPACE_DESCRIPTION, LOCAL_HOLON_SPACE_NAME, LOCAL_HOLON_SPACE_PATH,
};

use crate::get_holon_by_path;
use crate::guest_shared_objects::commit_functions;
use crate::persistence_layer::{
    delete_holon_node, delete_smartlink, expand_all_from_source, expand_from_source, get_holon,
    index_local_holon_space, persist_holon, saved_holon_from_stored,
};
use base_types::MapString;
use core_types::{HolonError, HolonId, HolonWriteRequest, SmartLink};
use holons_core::core_shared_objects::transactions::TransactionContextHandle;
use holons_core::{
    core_shared_objects::{transactions::TransactionContext, Holon, HolonCollection},
    reference_layer::{
        HolonCollectionApi, HolonReference, HolonServiceApi, SmartReference, WritableHolon,
    },
    StagedReference,
};
use holons_integrity::LinkTypes;
use holons_loader::HolonLoaderController;
use integrity_core_types::{LocalId, PropertyMap, PropertyName, RelationshipName};
use type_names::CoreRelationshipTypeName;

pub struct GuestHolonService;

impl GuestHolonService {
    pub fn new() -> Self {
        GuestHolonService
    }

    /// Creates and anchors the local space holon.
    ///
    /// This path deliberately goes transient → `persist_holon` without passing through staging.
    /// That is now load-bearing rather than incidental: space membership is stamped at staging
    /// time, so bypassing staging is what keeps the anchor out of its own `Owns` collection.
    /// Routing space creation through `stage_new_holon` would make the space own itself and put
    /// it back into whole-space discovery results.
    fn create_local_space_holon(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<SavedHolon, HolonError> {
        // Define the name and description for the local space holon
        let name: MapString = MapString(LOCAL_HOLON_SPACE_NAME.to_string());
        let description: MapString = MapString(LOCAL_HOLON_SPACE_DESCRIPTION.to_string());

        // Create new (empty) TransientHolon
        let mut space_holon_reference = context.mutation().new_holon(Some(name.clone()))?;
        space_holon_reference
            .with_property_value(
                PropertyName(MapString("name".to_string())),
                name.clone().into_base_value(),
            )?
            .with_property_value(
                PropertyName(MapString("description".to_string())),
                description.into_base_value(),
            )?;
        // Try to create the holon node in the DHT. The space holon begins its own lineage.
        let stored = persist_holon(HolonWriteRequest::PublishRoot {
            holon_node: space_holon_reference.into_model()?,
        })?;
        let saved_holon = saved_holon_from_stored(stored);

        // Retrieve the local ID for the holon
        let local_id = saved_holon.get_local_id()?;

        // Log the creation of the LocalHolonSpace
        info!("Created LocalHolonSpace with id {:#?}", local_id);

        // Anchor the persisted lineage root under the canonical LocalHolonSpace path.
        index_local_holon_space(&local_id)?;

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

        let handle = context.context_handle();

        Ok(HolonReference::Smart(SmartReference::new_from_id(handle, HolonId::Local(local_id))))
    }

    fn mint_smart_reference(
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

    /// Mints the runtime reference for one decoded SmartLink, paired with its optional key.
    ///
    /// Reads the canonical Tag v1 fields directly. Tag v1 encodes "keyless" as the empty
    /// canonical key and "no cached target properties" as an empty property map; both
    /// surface as `None` here.
    fn reference_from_smartlink(
        &self,
        context: &Arc<TransactionContext>,
        smartlink: &SmartLink,
    ) -> Result<(Option<MapString>, HolonReference), HolonError> {
        let key = (!smartlink.canonical_key.as_str().is_empty())
            .then(|| MapString(smartlink.canonical_key.as_str().to_string()));
        let smart_property_values = (!smartlink.target_property_values.is_empty())
            .then(|| smartlink.target_property_values.clone());

        let reference =
            self.mint_smart_reference(context, smartlink.target_id.clone(), smart_property_values)?;

        Ok((key, reference))
    }

    /// Retracts `local_id`'s space membership: its `OwnedBy` links and the reciprocal `Owns`
    /// links naming it on each owning space.
    ///
    /// The space's inverse view is retracted first because that is the side whole-space
    /// discovery reads; if only one direction could be retracted, it is the one that matters.
    ///
    /// Idempotent — `delete_smartlink` reports `AlreadyAbsent` rather than failing, so this is a
    /// no-op for a holon that never acquired membership or whose membership is already gone.
    fn retract_space_membership(local_id: &LocalId) -> Result<(), HolonError> {
        let owned_by = CoreRelationshipTypeName::OwnedBy.as_relationship_name();
        let owns = CoreRelationshipTypeName::Owns.as_relationship_name();

        for membership in expand_from_source(local_id, &owned_by)? {
            // Membership is a local-space relationship; a non-local owner is not something this
            // guest can retract, and is left for the space that authored it.
            let HolonId::Local(space_id) = &membership.target_id else {
                continue;
            };

            for member_link in expand_from_source(space_id, &owns)? {
                if member_link.target_id == HolonId::Local(local_id.clone()) {
                    delete_smartlink(&member_link.smartlink_id)?;
                }
            }

            delete_smartlink(&membership.smartlink_id)?;
        }

        Ok(())
    }
}

impl HolonServiceApi for GuestHolonService {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn commit_internal(
        &self,
        context: &Arc<TransactionContext>,
        staged_references: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        // Commit the staged holons provided by TransactionContext.
        let commit_response = commit_functions::commit(context, &staged_references)?;
        // Stage-clear policy is owned by TransactionContext.
        Ok(commit_response)
    }

    fn delete_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        local_id: &LocalId,
    ) -> Result<(), HolonError> {
        // Confirm the id names a real holon node before authoring a delete against it.
        let _stored = get_holon(local_id)?
            .ok_or_else(|| HolonError::HolonNotFound(format!("at id: {:?}", local_id.0)))?;
        // holon.is_deletable()?;

        // Membership is carried by SmartLinks, and deleting an entry does not retract the links
        // pointing at it, so a deleted holon would otherwise stay in its space's `Owns`
        // collection and remain discoverable. Retract membership before the node goes away.
        Self::retract_space_membership(local_id)?;

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

        let mut reference_map: HashMap<RelationshipName, Vec<(Option<MapString>, HolonReference)>> =
            HashMap::new();

        let smartlinks = expand_all_from_source(source_id.local_id())?;
        debug!("Retrieved {:?} smartlinks", smartlinks.len());

        for smartlink in smartlinks {
            let (canonical_key, reference) = self.reference_from_smartlink(context, &smartlink)?;

            // The following:
            // 1) adds an entry for relationship name if not already present (via `entry` API)
            // 2) adds a value for the entry if not already present (`.or_insert_with`)
            // 3) retains the tag's canonical key beside the reference to avoid hydrating the target

            reference_map
                .entry(smartlink.relationship_name)
                .or_insert_with(Vec::new)
                .push((canonical_key, reference));
        }

        for (map_name, holon_references) in reference_map {
            let mut collection = HolonCollection::new_existing();
            for (canonical_key, reference) in holon_references {
                let key = canonical_key.ok_or_else(|| {
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

    /// gets the exact HolonNode version named by the supplied id from the local persistent store,
    /// then "inflates" it into a Holon and returns it
    ///
    /// This is a version-addressed read: it returns the version that was asked for, not the head
    /// of that version's lineage.
    fn fetch_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        holon_id: &HolonId,
    ) -> Result<Holon, HolonError> {
        let local_id = Self::ensure_id_is_local(holon_id)?;

        match get_holon(&local_id)? {
            Some(stored) => Ok(Holon::Saved(saved_holon_from_stored(stored))),
            // No holon_node fetched for the specified holon_id
            None => Err(HolonError::HolonNotFound(local_id.to_string())),
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
        let smartlinks = expand_from_source(&local_id, relationship_name)?;
        debug!("Got {:?} smartlinks: {:#?}", smartlinks.len(), smartlinks);

        for smartlink in smartlinks {
            let (canonical_key, holon_reference) =
                self.reference_from_smartlink(context, &smartlink)?;
            collection.add_reference_with_key(canonical_key.as_ref(), &holon_reference)?;
        }
        Ok(collection)
    }

    /// Retrieves the holons owned by the transaction's current `HolonSpace`.
    ///
    /// Whole-space discovery is ordinary graph traversal: every committed lineage carries
    /// `OwnedBy → HolonSpace`, and commit materializes the reciprocal `Owns` SmartLink on the
    /// space, so the space's `Owns` collection *is* the membership list. Members are lineage
    /// roots, since membership is not re-emitted for later versions.
    ///
    /// The space anchor is not a member of its own collection; callers that need it use
    /// `TransactionContext::get_space_holon()`.
    fn get_all_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        // No space yet means nothing can belong to one. This is reachable before bootstrap has
        // anchored the local space, where an empty membership list is the right answer.
        let Some(space_reference) = context.get_space_holon()? else {
            return Ok(HolonCollection::new_existing());
        };

        self.fetch_related_holons_internal(
            context,
            &space_reference.holon_id()?,
            &CoreRelationshipTypeName::Owns.as_relationship_name(),
        )
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

    fn ensure_local_holon_space_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonReference, HolonError> {
        self.ensure_local_holon_space(context)
    }
}

impl fmt::Debug for GuestHolonService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuestHolonService").finish()
    }
}
