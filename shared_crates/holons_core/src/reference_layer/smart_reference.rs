use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{Arc, RwLock},
};
use tracing::{info, trace};
use type_names::relationship_names::CoreRelationshipTypeName;

use crate::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::{
        cache_access::HolonCacheAccess,
        holon::{state::AccessType, EssentialHolonContent, HolonCloneModel},
        relationship_behavior::ReadableRelationship,
        transient_holon_manager::ToHolonCloneModel,
        Holon, HolonCollection, ReadableHolonState,
    },
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, TransientReference, WritableHolon,
    },
    RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyMap, PropertyName, PropertyValue, RelationshipName,
};
use type_names::CorePropertyTypeName;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartReferenceSerializable {
    tx_id: TxId,
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl SmartReferenceSerializable {
    pub fn new(tx_id: TxId, holon_id: HolonId, smart_property_values: Option<PropertyMap>) -> Self {
        Self { tx_id, holon_id, smart_property_values }
    }

    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }
}

#[derive(new, Debug, Clone, PartialEq, Eq)]
pub struct SmartReference {
    context_handle: TransactionContextHandle,
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl SmartReference {
    // *************** CONSTRUCTORS ***************

    /// Constructor for SmartReference that takes a HolonId and sets smart_property_values to None
    pub fn new_from_id(context_handle: TransactionContextHandle, holon_id: HolonId) -> Self {
        SmartReference { context_handle, holon_id, smart_property_values: None }
    }

    pub fn new_with_properties(
        context_handle: TransactionContextHandle,
        holon_id: HolonId,
        smart_property_values: PropertyMap,
    ) -> Self {
        SmartReference { context_handle, holon_id, smart_property_values: Some(smart_property_values) }
    }

    /// Binds a wire reference to a TransactionContext, validating tx_id.
    pub fn bind(
        wire: SmartReferenceSerializable,
        context: Arc<TransactionContext>,
    ) -> Result<Self, HolonError> {
        if wire.tx_id != context.tx_id() {
            return Err(HolonError::CrossTransactionReference {
                reference_kind: "SmartReference".to_string(),
                reference_id: format!("HolonId={}", wire.holon_id),
                reference_tx: wire.tx_id.value(),
                context_tx: context.tx_id().value(),
            });
        }

        Ok(SmartReference {
            context_handle: TransactionContextHandle::new(context),
            holon_id: wire.holon_id,
            smart_property_values: wire.smart_property_values,
        })
    }

    // *************** ACCESSORS ***************

    /// Outside helper method for serialization purposes, that does not require a context.
    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }

    pub fn get_smart_properties(&self) -> Option<PropertyMap> {
        self.smart_property_values.clone()
    }

    // *************** UTILITY METHODS ***************

    fn get_cache_access(&self) -> Arc<dyn HolonCacheAccess> {
        self.context_handle.context().get_cache_access()
    }

    fn get_rc_holon(&self) -> Result<Arc<RwLock<Holon>>, HolonError> {
        // Get CacheAccess
        let cache_access = self.get_cache_access();

        // Retrieve the holon from the cache
        let rc_holon = cache_access.get_rc_holon(&self.holon_id)?;
        trace!("Got a reference to rc_holon from the cache manager: {:#?}", rc_holon);

        Ok(rc_holon)
    }

    // Simple string representations for errors/logging
    pub fn reference_kind_string(&self) -> String {
        "SmartReference".to_string()
    }

    pub fn reference_id_string(&self) -> String {
        format!("HolonId={}", self.holon_id)
    }
}

impl fmt::Display for SmartReference {
    /// Formats the `SmartReference` for human-readable display.
    ///
    /// The output includes the `HolonId` and a summary of smart property values:
    /// - If `smart_property_values` is `None` or empty, displays `"no props"`.
    /// - If there are 1–2 properties, displays them as key-value pairs.
    /// - If there are more than 2 properties, displays the first two followed by `"+N more"`.
    ///
    /// # Example Outputs
    /// - `SmartReference(Local(…ABC123), no props)`
    /// - `SmartReference(Local(…ABC123), props: [key1:value1, key2:value2])`
    /// - `SmartReference(Local(…ABC123), props: [key1:value1, key2:value2 +1 more])`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.smart_property_values {
            Some(props) if !props.is_empty() => {
                // Display the first 2 properties as a preview, followed by a count if there are more
                let preview: Vec<String> = props
                    .iter()
                    .take(2)
                    .map(|(key, value)| format!("{}:{:?}", key, value))
                    .collect();

                let additional_count = props.len().saturating_sub(preview.len());

                if additional_count > 0 {
                    write!(
                        f,
                        "SmartReference({}, props: [{} +{} more])",
                        self.holon_id,
                        preview.join(", "),
                        additional_count
                    )
                } else {
                    write!(f, "SmartReference({}, props: [{}])", self.holon_id, preview.join(", "))
                }
            }
            _ => write!(f, "SmartReference({}, no props)", self.holon_id),
        }
    }
}

impl ReadableHolonImpl for SmartReference {
    fn clone_holon_impl(&self) -> Result<TransientReference, HolonError> {
        self.is_accessible(AccessType::Clone)?;
        let transient_behavior = self.context_handle.context().get_transient_behavior_service();

        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for clone_holon_impl: {}",
                e
            ))
        })?;

        // HolonCloneModel for SavedHolon will have 'None' for relationships, as populating its RelationshipMap
        // is deferred to the reference layer, because context is needed that is only available in reference layer.
        let mut cloned_holon_transient_reference =
            transient_behavior.new_from_clone_model(borrowed_holon.holon_clone_model())?;

        let relationships = self.all_related_holons()?;
        let transient_relationships = relationships.clone_for_new_source()?;

        for (name, collection) in transient_relationships.map {
            let members = collection
                .read()
                .map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to acquire read lock on relationship collection in clone_holon_impl: {}",
                        e
                    ))
                })?
                .get_members()
                .to_vec();

            cloned_holon_transient_reference.add_related_holons(
                self.context_handle.context().as_ref(),
                name,
                members,
            )?;
        }

        Ok(cloned_holon_transient_reference)
    }

    fn all_related_holons_impl(&self) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let cache_access = self.get_cache_access();
        let relationship_map =
            cache_access.get_all_related_holons(self.context_handle.context().as_ref(), &self.get_id()?)?;

        Ok(relationship_map)
    }

    fn essential_content_impl(&self) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for essential_content_impl: {}",
                e
            ))
        })?;
        Ok(borrowed_holon.essential_content())
    }

    fn holon_id_impl(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }

    fn into_model_impl(&self) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for into_model_impl: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.into_node_model())
    }

    fn is_accessible_impl(&self, access_type: AccessType) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for is_accessible_impl: {}",
                e
            ))
        })?;
        holon.is_accessible(access_type)?;

        Ok(())
    }

    /// Extracts the Holon's primary key from `smart_property_values` or, if not found there,
    /// from its referenced holon. Returns `Ok(Some(MapString))` if found, or `Ok(None)` if absent.
    fn key_impl(&self) -> Result<Option<MapString>, HolonError> {
        let key_prop = CorePropertyTypeName::Key.as_property_name();

        match self.property_value_impl(&key_prop)? {
            Some(BaseValue::StringValue(s)) => Ok(Some(s.clone())),

            // Key exists but is wrong value type (e.g., enum?)
            Some(other) => Err(HolonError::InvalidType(format!(
                "Key property must be a StringValue, found {:?}",
                other
            ))),

            None => Ok(None),
        }
    }

    fn predecessor_impl(&self) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let collection_arc = self.related_holons(CoreRelationshipTypeName::Predecessor)?;
        let collection = collection_arc.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon collection in predecessor_impl: {}",
                e
            ))
        })?;
        collection.is_accessible(AccessType::Read)?;
        let members = collection.get_members();
        if members.len() > 1 {
            return Err(HolonError::Misc(format!(
                "related_holons for PREDECESSOR returned multiple members: {:#?}",
                members
            )));
        }
        if members.is_empty() {
            Ok(None)
        } else {
            Ok(Some(members[0].clone()))
        }
    }

    /// `property_value` returns the value for the specified property name
    /// It will attempt to get it from the smart_property_values map first to avoid having to
    /// retrieve the underlying holon. But, failing that, it will do a get_rc_holon from the cache
    /// manager in the context.
    ///
    /// Returns: Option, None if property for given name does not exist in its PropertyMap.
    fn property_value_impl(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        // Check if the property value is available in smart_property_values
        if let Some(smart_map) = &self.smart_property_values {
            if let Some(value) = smart_map.get(property_name) {
                return Ok(Some(value.clone()));
            }
        }

        info!("unable to get value for {:?} property from smart_property_values. Fetching rc_holon from HolonsCache", property_name);

        self.is_accessible(AccessType::Read)?;

        // Get rc_holon from HolonCacheManager
        let holon = self.get_rc_holon()?;
        let prop_val = holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on holon in property_value_impl: {}",
                    e
                ))
            })?
            .property_value(property_name)?;
        Ok(prop_val)
    }

    fn related_holons_impl(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        // Get CacheAccess
        let cache_access = self.get_cache_access();
        cache_access.get_related_holons(&self.holon_id, relationship_name)
    }

    fn summarize_impl(&self) -> Result<String, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for summarize_impl: {}",
                e
            ))
        })?;
        Ok(borrowed_holon.summarize())
    }

    fn versioned_key_impl(&self) -> Result<MapString, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let holon = self.get_rc_holon()?;
        let key = holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on holon for versioned_key_impl: {}",
                    e
                ))
            })?
            .versioned_key()?;

        Ok(key)
    }
}

// Convenience trait implementation for working with HolonReference wrappers.
// Functions will always fail since SmartReferences are immutable.
impl WritableHolonImpl for SmartReference {
    fn add_related_holons_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        _relationship_name: RelationshipName,
        _holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(self)
    }

    fn remove_related_holons_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        _relationship_name: RelationshipName,
        _holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(self)
    }

    fn with_property_value_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        _property: PropertyName,
        _value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(self)
    }

    fn remove_property_value_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        _name: PropertyName,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(self)
    }

    fn with_descriptor_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        _descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(())
    }

    fn with_predecessor_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        _predecessor_reference_option: Option<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(())
    }
}

impl ToHolonCloneModel for SmartReference {
    fn holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let model = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on holon for holon_clone_model: {}",
                    e
                ))
            })?
            .holon_clone_model();

        Ok(model)
    }
}
