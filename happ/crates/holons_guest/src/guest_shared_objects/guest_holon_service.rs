use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::{
    fmt,
    sync::{Arc, RwLock},
};

use hdk::prelude::*;
use holons_core::core_shared_objects::{ReadableHolonState, SavedHolon};
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
    delete_holon_node, expand_all_from_source, expand_from_source, expand_from_source_by_key,
    get_all_holon_ids, get_holon, index_local_holon_space, persist_holon, saved_holon_from_stored,
};
use base_types::{BaseValue, MapString};
use core_types::{CanonicalKey, HolonError, HolonId, HolonWriteRequest, KeyMatch, SmartLink};
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
use integrity_core_types::{
    LineageIntegrityReason, LocalId, PropertyMap, PropertyName, RelationshipName,
};
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

const CORE_SCHEMA_SPACE_KEY: &str = "MAP.CoreSchemaSpace";

pub struct GuestHolonService;

impl GuestHolonService {
    pub fn new() -> Self {
        GuestHolonService
    }

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
        let (key, smart_property_values) = smartlink_reference_properties(smartlink);

        let reference =
            self.mint_smart_reference(context, smartlink.target_id.clone(), smart_property_values)?;

        Ok((key, reference))
    }

    fn get_lineage_roots_by_key(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<Vec<LocalId>, HolonError> {
        let space = context.get_space_holon()?.ok_or_else(|| {
            HolonError::InvalidState("saved-holon lookup requires a persisted HolonSpace".into())
        })?;
        let space_id = space.holon_id()?.local_id().clone();
        let owns = CoreRelationshipTypeName::Owns.as_relationship_name();
        let canonical_key = CanonicalKey::new(key.0.clone())?;
        let links = expand_from_source_by_key(&space_id, &owns, KeyMatch::Exact(canonical_key))?;

        if links.is_empty() {
            return Err(HolonError::HolonNotFound(key.to_string()));
        }

        let mut roots = HashSet::new();
        for link in links {
            let HolonId::Local(target) = link.target_id else {
                return Err(HolonError::InvalidHolonReference(
                    "keyed Owns SmartLink target must be local".to_string(),
                ));
            };
            roots.insert(target);
        }

        let mut roots: Vec<_> = roots.into_iter().collect();
        roots.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(roots)
    }

    fn get_lineage_heads(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
        lineage_root: &LocalId,
        start: &LocalId,
    ) -> Result<Vec<LocalId>, HolonError> {
        fn traverse(
            holon_service: &GuestHolonService,
            context: &Arc<TransactionContext>,
            key: &MapString,
            lineage_root: &LocalId,
            current: LocalId,
            visited: &mut HashSet<LocalId>,
            active_path: &mut HashSet<LocalId>,
            heads: &mut Vec<LocalId>,
        ) -> Result<(), HolonError> {
            if !active_path.insert(current.clone()) {
                return Err(HolonError::LineageIntegrityError {
                    key: key.clone(),
                    lineage_root: lineage_root.clone(),
                    reason: LineageIntegrityReason::CycleDetected,
                });
            }

            let successors = holon_service.expand_smartlinks_from_source_internal(
                context,
                &current,
                &CoreRelationshipTypeName::Successor.as_relationship_name(),
            )?;
            if successors.is_empty() {
                heads.push(current.clone());
            } else {
                for successor in successors {
                    let HolonId::Local(target) = successor.target_id else {
                        return Err(HolonError::LineageIntegrityError {
                            key: key.clone(),
                            lineage_root: lineage_root.clone(),
                            reason: LineageIntegrityReason::MalformedSuccessorLink {
                                detail: "Successor SmartLink target must be local".to_string(),
                            },
                        });
                    };
                    if active_path.contains(&target) {
                        return Err(HolonError::LineageIntegrityError {
                            key: key.clone(),
                            lineage_root: lineage_root.clone(),
                            reason: LineageIntegrityReason::CycleDetected,
                        });
                    }
                    if visited.contains(&target) {
                        continue;
                    }
                    let stored =
                        get_holon(&target)?.ok_or_else(|| HolonError::LineageIntegrityError {
                            key: key.clone(),
                            lineage_root: lineage_root.clone(),
                            reason: LineageIntegrityReason::InvalidSuccessorTarget {
                                target: target.clone(),
                            },
                        })?;
                    if stored.version_metadata.lineage_root().as_local_id() != lineage_root {
                        return Err(HolonError::LineageIntegrityError {
                            key: key.clone(),
                            lineage_root: lineage_root.clone(),
                            reason: LineageIntegrityReason::InvalidSuccessorTarget { target },
                        });
                    }
                    traverse(
                        holon_service,
                        context,
                        key,
                        lineage_root,
                        target.clone(),
                        visited,
                        active_path,
                        heads,
                    )?;
                }
            }

            active_path.remove(&current);
            visited.insert(current);
            Ok(())
        }

        let mut visited = HashSet::new();
        let mut active_path = HashSet::new();
        let mut heads = Vec::new();
        traverse(
            self,
            context,
            key,
            lineage_root,
            start.clone(),
            &mut visited,
            &mut active_path,
            &mut heads,
        )?;
        Ok(heads)
    }

    fn get_lineage_heads_by_key(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<Vec<LocalId>, HolonError> {
        let lineage_roots = self.get_lineage_roots_by_key(context, key)?;
        let mut heads = HashSet::new();

        for lineage_root in lineage_roots {
            for head in self.get_lineage_heads(context, key, &lineage_root, &lineage_root)? {
                let stored =
                    get_holon(&head)?.ok_or_else(|| HolonError::HolonNotFound(head.to_string()))?;
                if saved_holon_from_stored(stored).key() == Some(key.clone()) {
                    heads.insert(head);
                }
            }
        }

        let mut heads: Vec<_> = heads.into_iter().collect();
        heads.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(heads)
    }
}

/// Produces the key and cached target properties for a SmartReference materialized from a
/// decoded SmartLink.
///
/// The canonical key is structurally authoritative SmartLink data. Carry it as the cached `Key`
/// property so the materialized SmartReference can answer `key()` without hydrating its target.
/// A keyless SmartLink keeps an absent `Key` property.
fn smartlink_reference_properties(
    smartlink: &SmartLink,
) -> (Option<MapString>, Option<PropertyMap>) {
    let key = (!smartlink.canonical_key.as_str().is_empty())
        .then(|| MapString(smartlink.canonical_key.as_str().to_string()));

    let mut smart_property_values = smartlink.target_property_values.clone();
    if let Some(key) = &key {
        let key_property_name = CorePropertyTypeName::Key.as_property_name();
        smart_property_values.insert(key_property_name, BaseValue::StringValue(key.clone()));
    }

    let smart_property_values =
        (!smart_property_values.is_empty()).then_some(smart_property_values);

    (key, smart_property_values)
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

        if context.is_bootstrap_provisioning() {
            self.complete_core_schema_bootstrap(context, staged_references, &commit_response)?;
        }
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
        let started_at = sys_time().ok().map(|timestamp| timestamp.as_micros());
        let result = match get_holon(&local_id)? {
            Some(stored) => Ok(Holon::Saved(saved_holon_from_stored(stored))),
            // No holon_node fetched for the specified holon_id
            None => Err(HolonError::HolonNotFound(local_id.to_string())),
        };

        if let (Some(started_at), Ok(completed_at)) = (started_at, sys_time()) {
            info!(
                "[PERF-674] persisted holon fetch: elapsed_ms={}",
                completed_at.as_micros().saturating_sub(started_at) / 1_000,
            );
        }

        result
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

    fn get_all_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        let mut collection = HolonCollection::new_existing();
        let holon_ids = get_all_holon_ids()?;
        let mut holon_references = Vec::new();
        for id in holon_ids {
            holon_references.push(self.mint_smart_reference(context, id, None)?);
        }
        collection.add_references(holon_references)?;

        Ok(collection)
    }

    fn expand_smartlinks_from_source_internal(
        &self,
        _context: &Arc<TransactionContext>,
        source_id: &LocalId,
        relationship_name: &RelationshipName,
    ) -> Result<Vec<SmartLink>, HolonError> {
        expand_from_source(source_id, relationship_name)
    }

    fn expand_smartlinks_from_source_by_key_internal(
        &self,
        _context: &Arc<TransactionContext>,
        source_id: &LocalId,
        relationship_name: &RelationshipName,
        key_match: KeyMatch,
    ) -> Result<Vec<SmartLink>, HolonError> {
        expand_from_source_by_key(source_id, relationship_name, key_match)
    }

    fn get_saved_holon_by_key_internal(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<SmartReference, HolonError> {
        let heads = self.get_lineage_heads_by_key(context, key)?;
        if heads.is_empty() {
            return Err(HolonError::HolonNotFound(key.to_string()));
        }
        let [head] = heads.as_slice() else {
            let mut lineage_roots = heads
                .iter()
                .map(|head| {
                    get_holon(head)
                        .and_then(|stored| {
                            stored.ok_or_else(|| HolonError::HolonNotFound(head.to_string()))
                        })
                        .map(|stored| stored.version_metadata.lineage_root().into_local_id())
                })
                .collect::<Result<HashSet<_>, _>>()?
                .into_iter()
                .collect::<Vec<_>>();
            lineage_roots.sort_by(|left, right| left.0.cmp(&right.0));
            return Err(HolonError::MultipleLineageHeads {
                key: key.clone(),
                lineage_roots,
                head_ids: heads,
            });
        };

        Ok(SmartReference::new_from_id(
            TransactionContextHandle::new(Arc::clone(context)),
            HolonId::Local(head.clone()),
        ))
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

impl GuestHolonService {
    /// Completes the one exceptional infrastructure action permitted to the
    /// bootstrap transaction after normal two-pass Commit has succeeded.
    fn complete_core_schema_bootstrap(
        &self,
        context: &Arc<TransactionContext>,
        staged_references: &[StagedReference],
        commit_response: &TransientReference,
    ) -> Result<(), HolonError> {
        let status =
            match commit_response.property_value(CorePropertyTypeName::CommitRequestStatus)? {
                Some(BaseValue::StringValue(value)) => value.0,
                Some(other) => {
                    return Err(HolonError::CommitFailure(format!(
                        "Core Schema bootstrap commit returned a non-string status: {other:?}"
                    )));
                }
                None => String::new(),
            };
        if status != "Complete" {
            return Err(HolonError::CommitFailure(
                "Core Schema bootstrap commit did not complete".to_string(),
            ));
        }

        let space = staged_references
            .iter()
            .find(|reference| {
                reference.key().ok().flatten().is_some_and(|key| key.0 == CORE_SCHEMA_SPACE_KEY)
            })
            .ok_or_else(|| {
                HolonError::CommitFailure(
                    "Core Schema bootstrap did not stage MAP.CoreSchemaSpace".to_string(),
                )
            })?;
        let space_id = space.holon_id()?.local_id().clone();

        index_local_holon_space(&space_id)?;
        context.set_space_holon_id(HolonId::Local(space_id))?;
        Ok(())
    }
}

impl fmt::Debug for GuestHolonService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuestHolonService").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{CanonicalKey, SmartLinkId, HOLOCHAIN_ACTION_HASH_BYTES};

    fn local_id(seed: u8) -> LocalId {
        LocalId(vec![seed; HOLOCHAIN_ACTION_HASH_BYTES])
    }

    fn smartlink(canonical_key: &str, target_property_values: PropertyMap) -> SmartLink {
        SmartLink {
            smartlink_id: SmartLinkId(local_id(1)),
            source_id: local_id(2),
            target_id: HolonId::Local(local_id(3)),
            relationship_name: RelationshipName(MapString("Related".to_string())),
            canonical_key: CanonicalKey::new(canonical_key).expect("test canonical key is valid"),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_values,
        }
    }

    #[test]
    fn smartlink_reference_properties_adds_canonical_key_to_cached_properties() {
        let (key, properties) =
            smartlink_reference_properties(&smartlink("book-1", PropertyMap::new()));

        assert_eq!(key, Some(MapString("book-1".to_string())));
        assert_eq!(
            properties
                .expect("keyed SmartLink should produce cached properties")
                .get(&CorePropertyTypeName::Key.as_property_name()),
            Some(&BaseValue::StringValue(MapString("book-1".to_string())))
        );
    }

    #[test]
    fn smartlink_reference_properties_preserves_cached_target_properties() {
        let title_property = PropertyName(MapString("Title".to_string()));
        let mut target_properties = PropertyMap::new();
        target_properties.insert(
            title_property.clone(),
            BaseValue::StringValue(MapString("The Book".to_string())),
        );

        let (_, properties) =
            smartlink_reference_properties(&smartlink("book-1", target_properties));
        let properties = properties.expect("keyed SmartLink should produce cached properties");

        assert_eq!(
            properties.get(&title_property),
            Some(&BaseValue::StringValue(MapString("The Book".to_string())))
        );
        assert_eq!(
            properties.get(&CorePropertyTypeName::Key.as_property_name()),
            Some(&BaseValue::StringValue(MapString("book-1".to_string())))
        );
    }

    #[test]
    fn smartlink_reference_properties_keeps_keyless_smartlinks_keyless() {
        let (key, properties) = smartlink_reference_properties(&smartlink("", PropertyMap::new()));

        assert_eq!(key, None);
        assert_eq!(properties, None);
    }
}
